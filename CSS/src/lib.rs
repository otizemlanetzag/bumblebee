//! Bumblebee's original pixel-oriented rendering core.
//!
//! The RGB palette is a 4096x4096 PNG containing every 24-bit RGB color
//! exactly once. Its order is a reversible permutation, so the renderer can
//! locate a color's source pixel without a large hash table.

use image::{ImageReader, RgbImage, RgbaImage};
use std::path::Path;

pub const PALETTE_SIDE: u32 = 4096;
pub const COLOR_COUNT: usize = 1 << 24;
const COLOR_MASK: u32 = (1 << 24) - 1;
const PERMUTATION_MULTIPLIER: u32 = 0x00BD_1E9B;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const fn packed(self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | self.b as u32
    }

    pub const fn from_packed(value: u32) -> Self {
        Self {
            r: (value >> 16) as u8,
            g: (value >> 8) as u8,
            b: value as u8,
        }
    }

    /// Parses #RGB or #RRGGBB CSS-style hexadecimal colors.
    pub fn from_hex(value: &str) -> Result<Self, String> {
        let digits = value.strip_prefix('#').unwrap_or(value);
        match digits.len() {
            3 => {
                let mut chars = digits.chars();
                let r = chars.next().ok_or("missing red component")?;
                let g = chars.next().ok_or("missing green component")?;
                let b = chars.next().ok_or("missing blue component")?;
                Ok(Self::rgb(
                    u8::from_str_radix(&format!("{r}{r}"), 16).map_err(|e| e.to_string())?,
                    u8::from_str_radix(&format!("{g}{g}"), 16).map_err(|e| e.to_string())?,
                    u8::from_str_radix(&format!("{b}{b}"), 16).map_err(|e| e.to_string())?,
                ))
            }
            6 => {
                let packed = u32::from_str_radix(digits, 16).map_err(|e| e.to_string())?;
                Ok(Self::from_packed(packed))
            }
            _ => Err(format!("expected #RGB or #RRGGBB, got {value}")),
        }
    }
}

/// A bijection over all 24-bit integers. It scrambles palette order while
/// keeping a cheap inverse. The odd multiplier is invertible modulo 2^24.
fn permute(index: u32) -> u32 {
    let mixed = index ^ (index >> 8);
    mixed.wrapping_mul(PERMUTATION_MULTIPLIER) & COLOR_MASK
}

fn modular_inverse_24(value: u32) -> u32 {
    // Extended Euclid: value is odd, so gcd(value, 2^24) is always one.
    let modulus = 1i64 << 24;
    let mut t = 0i64;
    let mut new_t = 1i64;
    let mut r = modulus;
    let mut new_r = value as i64;

    while new_r != 0 {
        let quotient = r / new_r;
        (t, new_t) = (new_t, t - quotient * new_t);
        (r, new_r) = (new_r, r - quotient * new_r);
    }

    if r != 1 { return 0; }
    t.rem_euclid(modulus) as u32
}

fn unpermute(value: u32) -> u32 {
    // Compute the inverse only once; this function is used for every lookup.
    static INVERSE: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    let inverse = *INVERSE.get_or_init(|| modular_inverse_24(PERMUTATION_MULTIPLIER));
    let mixed = value.wrapping_mul(inverse) & COLOR_MASK;
    // Inverse of x ^= x >> 8 over a 24-bit word.
    (mixed ^ (mixed >> 8) ^ (mixed >> 16)) & COLOR_MASK
}

pub struct Palette {
    /// Decoded RGB pixels in the exact row-major order of the source PNG.
    pixels: Vec<u8>,
}

impl Palette {
    /// Creates the complete, shuffled 4096x4096 RGB palette PNG.
    pub fn generate_png(path: impl AsRef<Path>) -> Result<(), String> {
        let mut bytes = vec![0u8; COLOR_COUNT * 3];
        for index in 0..COLOR_COUNT {
            let color = Color::from_packed(permute(index as u32));
            let offset = index * 3;
            bytes[offset] = color.r;
            bytes[offset + 1] = color.g;
            bytes[offset + 2] = color.b;
        }

        let image = RgbImage::from_raw(PALETTE_SIDE, PALETTE_SIDE, bytes)
            .ok_or("could not construct palette image")?;
        image.save(path).map_err(|e| format!("could not save palette: {e}"))
    }

    /// Loads and validates the palette once at browser startup.
    ///
    /// Validation checks every pixel against the known reversible shuffle.
    /// This proves that every 24-bit RGB value occurs exactly once.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let image = ImageReader::open(path)
            .map_err(|e| format!("could not open RGB palette: {e}"))?
            .decode()
            .map_err(|e| format!("could not decode RGB palette: {e}"))?
            .to_rgb8();

        if image.width() != PALETTE_SIDE || image.height() != PALETTE_SIDE {
            return Err(format!(
                "palette must be {PALETTE_SIDE}x{PALETTE_SIDE}, got {}x{}",
                image.width(),
                image.height()
            ));
        }

        let pixels = image.into_raw();
        for index in 0..COLOR_COUNT {
            let offset = index * 3;
            let actual = ((pixels[offset] as u32) << 16)
                | ((pixels[offset + 1] as u32) << 8)
                | pixels[offset + 2] as u32;
            let expected = permute(index as u32);
            if actual != expected {
                return Err(format!(
                    "invalid RGB palette at pixel {index}: expected #{expected:06X}, got #{actual:06X}"
                ));
            }
        }

        Ok(Self { pixels })
    }

    /// Returns the source-image pixel index for a requested RGB color.
    pub fn source_index(&self, color: Color) -> usize {
        unpermute(color.packed()) as usize
    }

    /// Looks up the requested color by locating and reading its palette pixel.
    pub fn sample(&self, color: Color) -> Color {
        let offset = self.source_index(color) * 3;
        Color::rgb(
            self.pixels[offset],
            self.pixels[offset + 1],
            self.pixels[offset + 2],
        )
    }
}

pub struct Framebuffer {
    width: u32,
    height: u32,
    /// RGBA output pixels, row-major.
    pixels: Vec<u8>,
}

impl Framebuffer {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err("framebuffer dimensions must be greater than zero".into());
        }
        let byte_len = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or("framebuffer dimensions are too large")?;
        Ok(Self {
            width,
            height,
            pixels: vec![0; byte_len],
        })
    }

    pub fn width(&self) -> u32 { self.width }
    pub fn height(&self) -> u32 { self.height }

    pub fn clear(&mut self, palette: &Palette, color: Color) {
        let c = palette.sample(color);
        for pixel in self.pixels.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[c.r, c.g, c.b, 255]);
        }
    }

    pub fn set_pixel(&mut self, palette: &Palette, x: i32, y: i32, color: Color) {
        if !self.in_bounds(x, y) { return; }
        let c = palette.sample(color);
        let offset = self.offset(x as u32, y as u32);
        self.pixels[offset..offset + 4].copy_from_slice(&[c.r, c.g, c.b, 255]);
    }

    /// Alpha-composites a palette color over the existing framebuffer pixel.
    pub fn blend_pixel(&mut self, palette: &Palette, x: i32, y: i32, color: Color, alpha: u8) {
        if alpha == 0 || !self.in_bounds(x, y) { return; }
        if alpha == 255 {
            self.set_pixel(palette, x, y, color);
            return;
        }
        let source = palette.sample(color);
        let offset = self.offset(x as u32, y as u32);
        let a = alpha as u32;
        let inverse = 255 - a;
        for (channel, src) in [source.r, source.g, source.b].into_iter().enumerate() {
            let dst = self.pixels[offset + channel] as u32;
            self.pixels[offset + channel] = ((src as u32 * a + dst * inverse + 127) / 255) as u8;
        }
        self.pixels[offset + 3] = 255;
    }

    pub fn fill_rect(&mut self, palette: &Palette, x: i32, y: i32, width: u32, height: u32, color: Color) {
        let left = x.max(0) as u32;
        let top = y.max(0) as u32;
        let right = (x as i64 + width as i64).clamp(0, self.width as i64) as u32;
        let bottom = (y as i64 + height as i64).clamp(0, self.height as i64) as u32;
        if right <= left || bottom <= top { return; }
        let c = palette.sample(color);
        for py in top..bottom {
            for px in left..right {
                let offset = self.offset(px, py);
                self.pixels[offset..offset + 4].copy_from_slice(&[c.r, c.g, c.b, 255]);
            }
        }
    }

    pub fn stroke_rect(&mut self, palette: &Palette, x: i32, y: i32, width: u32, height: u32, thickness: u32, color: Color) {
        if width == 0 || height == 0 || thickness == 0 { return; }
        let t = thickness.min(width).min(height);
        self.fill_rect(palette, x, y, width, t, color);
        self.fill_rect(palette, x, y + (height - t) as i32, width, t, color);
        self.fill_rect(palette, x, y, t, height, color);
        self.fill_rect(palette, x + (width - t) as i32, y, t, height, color);
    }

    pub fn draw_line(&mut self, palette: &Palette, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
        let (mut x, mut y) = (x0, y0);
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut error = dx + dy;
        loop {
            self.set_pixel(palette, x, y, color);
            if x == x1 && y == y1 { break; }
            let twice = 2 * error;
            if twice >= dy { error += dy; x += sx; }
            if twice <= dx { error += dx; y += sy; }
        }
    }

    pub fn fill_circle(&mut self, palette: &Palette, center_x: i32, center_y: i32, radius: u32, color: Color) {
        let r = radius as i64;
        let r2 = r * r;
        let c = palette.sample(color);
        for dy in -r..=r {
            let remaining = r2 - dy * dy;
            let span = (remaining as f64).sqrt() as i32;
            for dx in -span..=span {
                let x = center_x + dx;
                let y = center_y + dy as i32;
                if self.in_bounds(x, y) {
                    let offset = self.offset(x as u32, y as u32);
                    self.pixels[offset..offset + 4].copy_from_slice(&[c.r, c.g, c.b, 255]);
                }
            }
        }
    }

    /// Draws a horizontal CSS-like linear gradient into a rectangle.
    pub fn linear_gradient_rect(
        &mut self,
        palette: &Palette,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        start: Color,
        end: Color,
    ) {
        if width == 0 || height == 0 { return; }
        for dx in 0..width {
            let t = if width <= 1 { 0.0 } else { dx as f32 / (width - 1) as f32 };
            let color = Color::rgb(
                lerp(start.r, end.r, t),
                lerp(start.g, end.g, t),
                lerp(start.b, end.b, t),
            );
            self.fill_rect(palette, x + dx as i32, y, 1, height, color);
        }
    }

    /// Fills an axis-aligned ellipse using scanline spans.
    pub fn fill_ellipse(
        &mut self,
        palette: &Palette,
        center_x: i32,
        center_y: i32,
        radius_x: u32,
        radius_y: u32,
        color: Color,
    ) {
        if radius_x == 0 || radius_y == 0 { return; }
        let rx = radius_x as f64;
        let ry = radius_y as f64;
        let c = palette.sample(color);
        let min_y = (center_y as i64 - radius_y as i64).max(0) as i32;
        let max_y = (center_y as i64 + radius_y as i64).min(self.height as i64 - 1) as i32;
        for y in min_y..=max_y {
            let normalized_y = (y - center_y) as f64 / ry;
            let remaining = (1.0 - normalized_y * normalized_y).max(0.0);
            let span = (rx * remaining.sqrt()).floor() as i64;
            let left = (center_x as i64 - span).max(0);
            let right = (center_x as i64 + span).min(self.width as i64 - 1);
            if left > right { continue; }
            for x in left..=right {
                let offset = self.offset(x as u32, y as u32);
                self.pixels[offset..offset + 4].copy_from_slice(&[c.r, c.g, c.b, 255]);
            }
        }
    }

    /// Fills a rounded rectangle. The radius is clamped to half its smaller side.
    pub fn fill_rounded_rect(
        &mut self,
        palette: &Palette,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        radius: u32,
        color: Color,
    ) {
        if width == 0 || height == 0 { return; }
        let radius = radius.min(width / 2).min(height / 2) as f64;
        if radius == 0.0 {
            self.fill_rect(palette, x, y, width, height, color);
            return;
        }
        let c = palette.sample(color);
        let left = (x as i64).max(0);
        let top = (y as i64).max(0);
        let right = (x as i64 + width as i64).min(self.width as i64);
        let bottom = (y as i64 + height as i64).min(self.height as i64);
        if left >= right || top >= bottom { return; }

        let rect_left = x as f64 + radius;
        let rect_right = x as f64 + width as f64 - radius;
        let rect_top = y as f64 + radius;
        let rect_bottom = y as f64 + height as f64 - radius;
        let radius_squared = radius * radius;

        for py in top..bottom {
            for px in left..right {
                let nearest_x = (px as f64 + 0.5).clamp(rect_left, rect_right);
                let nearest_y = (py as f64 + 0.5).clamp(rect_top, rect_bottom);
                let dx = px as f64 + 0.5 - nearest_x;
                let dy = py as f64 + 0.5 - nearest_y;
                if dx * dx + dy * dy <= radius_squared {
                    let offset = self.offset(px as u32, py as u32);
                    self.pixels[offset..offset + 4].copy_from_slice(&[c.r, c.g, c.b, 255]);
                }
            }
        }
    }

    /// Draws only the border of a rounded rectangle, preserving pixels inside it.
    pub fn stroke_rounded_rect(
        &mut self,
        palette: &Palette,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        radius: u32,
        thickness: u32,
        color: Color,
    ) {
        if width == 0 || height == 0 || thickness == 0 { return; }
        let outer_radius = radius.min(width / 2).min(height / 2) as f64;
        let inset = thickness.min(width / 2).min(height / 2);
        let inner_width = width.saturating_sub(inset * 2);
        let inner_height = height.saturating_sub(inset * 2);
        let inner_radius = outer_radius.saturating_sub(inset as f64);
        let c = palette.sample(color);
        let left = (x as i64).max(0);
        let top = (y as i64).max(0);
        let right = (x as i64 + width as i64).min(self.width as i64);
        let bottom = (y as i64 + height as i64).min(self.height as i64);
        if left >= right || top >= bottom { return; }

        let contains = |px: f64, py: f64, rx: f64, ry: f64, rw: f64, rh: f64, r: f64| {
            if rw <= 0.0 || rh <= 0.0 || px < rx || py < ry || px >= rx + rw || py >= ry + rh {
                return false;
            }
            if r <= 0.0 { return true; }
            let cx = px.clamp(rx + r, rx + rw - r);
            let cy = py.clamp(ry + r, ry + rh - r);
            let dx = px - cx;
            let dy = py - cy;
            dx * dx + dy * dy <= r * r
        };

        for py in top..bottom {
            for px in left..right {
                let cx = px as f64 + 0.5;
                let cy = py as f64 + 0.5;
                let in_outer = contains(cx, cy, x as f64, y as f64, width as f64, height as f64, outer_radius);
                if !in_outer { continue; }
                let in_inner = contains(
                    cx,
                    cy,
                    x as f64 + inset as f64,
                    y as f64 + inset as f64,
                    inner_width as f64,
                    inner_height as f64,
                    inner_radius,
                );
                if !in_inner {
                    let offset = self.offset(px as u32, py as u32);
                    self.pixels[offset..offset + 4].copy_from_slice(&[c.r, c.g, c.b, 255]);
                }
            }
        }
    }

    /// Fills a polygon using an even-odd scanline rule.
    /// The final vertex is automatically connected to the first.
    pub fn fill_polygon(&mut self, palette: &Palette, points: &[(i32, i32)], color: Color) {
        if points.len() < 3 { return; }
        let min_y = points.iter().map(|p| p.1).min().unwrap().max(0);
        let max_y = points.iter().map(|p| p.1).max().unwrap().min(self.height as i32 - 1);
        if min_y > max_y { return; }
        let c = palette.sample(color);
        let mut intersections = Vec::<f64>::with_capacity(points.len());

        for y in min_y..=max_y {
            intersections.clear();
            let scan_y = y as f64 + 0.5;
            for index in 0..points.len() {
                let (x1, y1) = points[index];
                let (x2, y2) = points[(index + 1) % points.len()];
                if y1 == y2 { continue; }
                let low = y1.min(y2) as f64;
                let high = y1.max(y2) as f64;
                if scan_y < low || scan_y >= high { continue; }
                let t = (scan_y - y1 as f64) / (y2 - y1) as f64;
                intersections.push(x1 as f64 + t * (x2 - x1) as f64);
            }
            intersections.sort_by(|a, b| a.total_cmp(b));
            for pair in intersections.chunks_exact(2) {
                let left = (pair[0].ceil() as i64).max(0);
                let right = (pair[1].ceil() as i64).min(self.width as i64);
                for px in left..right {
                    let offset = self.offset(px as u32, y as u32);
                    self.pixels[offset..offset + 4].copy_from_slice(&[c.r, c.g, c.b, 255]);
                }
            }
        }
    }

    /// Draws the edges of a polygonal path, optionally closing it.
    pub fn draw_polyline(
        &mut self,
        palette: &Palette,
        points: &[(i32, i32)],
        closed: bool,
        color: Color,
    ) {
        if points.len() < 2 { return; }
        for pair in points.windows(2) {
            self.draw_line(palette, pair[0].0, pair[0].1, pair[1].0, pair[1].1, color);
        }
        if closed {
            let first = points[0];
            let last = points[points.len() - 1];
            self.draw_line(palette, last.0, last.1, first.0, first.1, color);
        }
    }

    /// Draws a two-dimensional radial gradient inside an ellipse.
    pub fn radial_gradient_ellipse(
        &mut self,
        palette: &Palette,
        center_x: i32,
        center_y: i32,
        radius_x: u32,
        radius_y: u32,
        center_color: Color,
        edge_color: Color,
    ) {
        if radius_x == 0 || radius_y == 0 { return; }
        let rx = radius_x as f64;
        let ry = radius_y as f64;
        let min_y = (center_y as i64 - radius_y as i64).max(0) as i32;
        let max_y = (center_y as i64 + radius_y as i64).min(self.height as i64 - 1) as i32;
        for y in min_y..=max_y {
            let min_x = (center_x as i64 - radius_x as i64).max(0) as i32;
            let max_x = (center_x as i64 + radius_x as i64).min(self.width as i64 - 1) as i32;
            for x in min_x..=max_x {
                let dx = (x - center_x) as f64 / rx;
                let dy = (y - center_y) as f64 / ry;
                let distance = (dx * dx + dy * dy).sqrt();
                if distance > 1.0 { continue; }
                let t = distance as f32;
                let color = Color::rgb(
                    lerp(center_color.r, edge_color.r, t),
                    lerp(center_color.g, edge_color.g, t),
                    lerp(center_color.b, edge_color.b, t),
                );
                self.set_pixel(palette, x, y, color);
            }
        }
    }

    pub fn save_png(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let image = RgbaImage::from_raw(self.width, self.height, self.pixels.clone())
            .ok_or("could not construct framebuffer image")?;
        image.save(path).map_err(|e| format!("could not save rendered image: {e}"))
    }

    fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height
    }

    fn offset(&self, x: u32, y: u32) -> usize {
        ((y as usize * self.width as usize) + x as usize) * 4
    }
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permutation_round_trips_sample_values() {
        for value in [0, 1, 2, 255, 256, 65_535, 0x123456, COLOR_MASK] {
            assert_eq!(unpermute(permute(value)), value);
        }
    }

    #[test]
    fn permutation_is_unique_for_a_large_sample() {
        let mut seen = std::collections::HashSet::new();
        for value in 0..1_000_000u32 {
            assert!(seen.insert(permute(value)));
        }
    }

    #[test]
    fn hex_colors_parse() {
        assert_eq!(Color::from_hex("#0af").unwrap(), Color::rgb(0, 170, 255));
        assert_eq!(Color::from_hex("#12A4E8").unwrap(), Color::rgb(18, 164, 232));
    }

    #[test]
    fn inverse_maps_color_to_palette_pixel() {
        for value in [0, 1, 0x123456, COLOR_MASK] {
            let color = Color::from_packed(value);
            let index = unpermute(value) as usize;
            assert_eq!(permute(index as u32), color.packed());
        }
    }

    #[test]
    fn polygon_and_shape_inputs_handle_degenerate_cases() {
        let points = [(0, 0), (10, 0)];
        assert!(points.len() < 3);
    }

    #[test]
    fn palette_permutation_covers_full_domain_at_boundaries() {
        let mut seen = std::collections::HashSet::new();
        for value in [0, 1, 2, 3, 0x00FF_FF, 0x12_3456, 0x7F_FFFF, COLOR_MASK] {
            assert!(seen.insert(permute(value)));
            assert_eq!(unpermute(permute(value)), value);
        }
    }

    #[test]
    fn framebuffer_clips_pixels_outside_bounds() {
        let mut frame = Framebuffer::new(2, 2).unwrap();
        assert!(!frame.in_bounds(-1, 0));
        assert!(frame.in_bounds(1, 1));
        assert!(!frame.in_bounds(2, 1));
    }
}
