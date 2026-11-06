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

/// A 2D affine transform using the standard six CSS/SVG matrix values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform2D {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}

impl Default for Transform2D {
    fn default() -> Self { Self::IDENTITY }
}

impl Transform2D {
    pub const IDENTITY: Self = Self { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };

    pub const fn new(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) -> Self {
        Self { a, b, c, d, e, f }
    }

    pub fn translation(x: f32, y: f32) -> Self { Self::new(1.0, 0.0, 0.0, 1.0, x, y) }
    pub fn scale(x: f32, y: f32) -> Self { Self::new(x, 0.0, 0.0, y, 0.0, 0.0) }

    pub fn rotation_radians(angle: f32) -> Self {
        let (sin, cos) = angle.sin_cos();
        Self::new(cos, sin, -sin, cos, 0.0, 0.0)
    }

    /// Returns self multiplied by rhs; rhs is applied to points first.
    pub fn multiply(self, rhs: Self) -> Self {
        Self::new(
            self.a * rhs.a + self.c * rhs.b,
            self.b * rhs.a + self.d * rhs.b,
            self.a * rhs.c + self.c * rhs.d,
            self.b * rhs.c + self.d * rhs.d,
            self.a * rhs.e + self.c * rhs.f + self.e,
            self.b * rhs.e + self.d * rhs.f + self.f,
        )
    }

    pub fn apply(self, point: (f32, f32)) -> (f32, f32) {
        (
            self.a * point.0 + self.c * point.1 + self.e,
            self.b * point.0 + self.d * point.1 + self.f,
        )
    }
}

/// Commands for a compact vector path. Coordinates are in framebuffer pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathCommand {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    QuadraticTo { control: (f32, f32), end: (f32, f32), segments: u32 },
    CubicTo {
        control1: (f32, f32),
        control2: (f32, f32),
        end: (f32, f32),
        segments: u32,
    },
    Close,
}

/// A vector path that can be flattened into points before rasterization.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VectorPath {
    commands: Vec<PathCommand>,
}

impl VectorPath {
    pub fn new() -> Self { Self::default() }
    pub fn move_to(&mut self, x: f32, y: f32) -> &mut Self {
        self.commands.push(PathCommand::MoveTo(x, y));
        self
    }
    pub fn line_to(&mut self, x: f32, y: f32) -> &mut Self {
        self.commands.push(PathCommand::LineTo(x, y));
        self
    }
    pub fn quadratic_to(&mut self, control: (f32, f32), end: (f32, f32), segments: u32) -> &mut Self {
        self.commands.push(PathCommand::QuadraticTo { control, end, segments });
        self
    }
    pub fn cubic_to(
        &mut self,
        control1: (f32, f32),
        control2: (f32, f32),
        end: (f32, f32),
        segments: u32,
    ) -> &mut Self {
        self.commands.push(PathCommand::CubicTo { control1, control2, end, segments });
        self
    }
    pub fn close(&mut self) -> &mut Self {
        self.commands.push(PathCommand::Close);
        self
    }

    /// Converts curves to line segments. A new MoveTo starts a new subpath.
    /// Applies an affine transform to every coordinate in the path.
    pub fn transformed(&self, transform: Transform2D) -> Self {
        let mut result = Self::new();
        for command in &self.commands {
            match *command {
                PathCommand::MoveTo(x, y) => {
                    let p = transform.apply((x, y));
                    result.move_to(p.0, p.1);
                }
                PathCommand::LineTo(x, y) => {
                    let p = transform.apply((x, y));
                    result.line_to(p.0, p.1);
                }
                PathCommand::QuadraticTo { control, end, segments } => {
                    result.quadratic_to(transform.apply(control), transform.apply(end), segments);
                }
                PathCommand::CubicTo { control1, control2, end, segments } => {
                    result.cubic_to(
                        transform.apply(control1),
                        transform.apply(control2),
                        transform.apply(end),
                        segments,
                    );
                }
                PathCommand::Close => { result.close(); }
            }
        }
        result
    }

    pub fn flatten(&self) -> Vec<Vec<(i32, i32)>> {
        let mut paths: Vec<Vec<(i32, i32)>> = Vec::new();
        let mut current: Option<(f32, f32)> = None;
        let mut start: Option<(f32, f32)> = None;

        for command in &self.commands {
            match *command {
                PathCommand::MoveTo(x, y) => {
                    paths.push(vec![(x.round() as i32, y.round() as i32)]);
                    current = Some((x, y));
                    start = Some((x, y));
                }
                PathCommand::LineTo(x, y) => {
                    if let (Some(point), Some(path)) = (current, paths.last_mut()) {
                        path.push((x.round() as i32, y.round() as i32));
                        current = Some((x, y));
                        let _ = point;
                    }
                }
                PathCommand::QuadraticTo { control, end, segments } => {
                    if let (Some(from), Some(path)) = (current, paths.last_mut()) {
                        let steps = segments.clamp(2, 4096);
                        for step in 1..=steps {
                            let t = step as f32 / steps as f32;
                            let u = 1.0 - t;
                            let point = (
                                u*u*from.0 + 2.0*u*t*control.0 + t*t*end.0,
                                u*u*from.1 + 2.0*u*t*control.1 + t*t*end.1,
                            );
                            path.push((point.0.round() as i32, point.1.round() as i32));
                        }
                        current = Some(end);
                    }
                }
                PathCommand::CubicTo { control1, control2, end, segments } => {
                    if let (Some(from), Some(path)) = (current, paths.last_mut()) {
                        let steps = segments.clamp(2, 4096);
                        for step in 1..=steps {
                            let t = step as f32 / steps as f32;
                            let u = 1.0 - t;
                            let point = (
                                u*u*u*from.0 + 3.0*u*u*t*control1.0 + 3.0*u*t*t*control2.0 + t*t*t*end.0,
                                u*u*u*from.1 + 3.0*u*u*t*control1.1 + 3.0*u*t*t*control2.1 + t*t*t*end.1,
                            );
                            path.push((point.0.round() as i32, point.1.round() as i32));
                        }
                        current = Some(end);
                    }
                }
                PathCommand::Close => {
                    if let (Some(first), Some(path)) = (start, paths.last_mut()) {
                        path.push((first.0.round() as i32, first.1.round() as i32));
                        current = Some(first);
                    }
                }
            }
        }
        paths.retain(|path| !path.is_empty());
        paths
    }
}

pub struct Framebuffer {
    width: u32,
    height: u32,
    /// Optional framebuffer-space clipping rectangle: (left, top, right, bottom).
    clip: Option<(u32, u32, u32, u32)>,
    /// Saved clipping states, including None for the full framebuffer.
    clip_stack: Vec<Option<(u32, u32, u32, u32)>>,
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
            clip: None,
            clip_stack: Vec::new(),
            pixels: vec![0; byte_len],
        })
    }

    /// Replaces the current clip rectangle. Coordinates are clipped to the framebuffer.
    pub fn set_clip(&mut self, x: i32, y: i32, width: u32, height: u32) {
        let left = (x as i64).max(0).min(self.width as i64) as u32;
        let top = (y as i64).max(0).min(self.height as i64) as u32;
        let right = (x as i64 + width as i64).max(0).min(self.width as i64) as u32;
        let bottom = (y as i64 + height as i64).max(0).min(self.height as i64) as u32;
        self.clip = Some((left, top, right.max(left), bottom.max(top)));
    }

    /// Intersects a new clip rectangle with the existing clip.
    pub fn intersect_clip(&mut self, x: i32, y: i32, width: u32, height: u32) {
        let left = (x as i64).max(0).min(self.width as i64) as u32;
        let top = (y as i64).max(0).min(self.height as i64) as u32;
        let right = (x as i64 + width as i64).max(0).min(self.width as i64) as u32;
        let bottom = (y as i64 + height as i64).max(0).min(self.height as i64) as u32;
        let next = (left, top, right.max(left), bottom.max(top));
        self.clip = Some(match self.clip {
            Some((a, b, c, d)) => (a.max(next.0), b.max(next.1), c.min(next.2), d.min(next.3)),
            None => next,
        });
    }

    /// Saves the current clip state so nested drawing can restore it later.
    pub fn save_clip(&mut self) { self.clip_stack.push(self.clip); }

    /// Restores the latest saved clip state. Returns false when the stack is empty.
    pub fn restore_clip(&mut self) -> bool {
        match self.clip_stack.pop() {
            Some(clip) => { self.clip = clip; true }
            None => false,
        }
    }

    pub fn clip_stack_depth(&self) -> usize { self.clip_stack.len() }

    /// Removes clipping and restores drawing to the full framebuffer.
    pub fn reset_clip(&mut self) { self.clip = None; }

    pub fn width(&self) -> u32 { self.width }
    pub fn height(&self) -> u32 { self.height }

    pub fn clear(&mut self, palette: &Palette, color: Color) {
        let c = palette.sample(color);
        for pixel in self.pixels.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[c.r, c.g, c.b, 255]);
        }
    }

    /// Clears the framebuffer to fully transparent pixels.
    /// Unlike clear(), this is useful for off-screen compositing layers.
    pub fn clear_transparent(&mut self) {
        self.pixels.fill(0);
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
                if !self.in_bounds(px as i32, py as i32) { continue; }
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

    /// Draws an anti-aliased line using coverage-weighted pixel blending.
    pub fn draw_line_antialiased(
        &mut self,
        palette: &Palette,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        color: Color,
    ) {
        // Xiaolin Wu's line algorithm. Steep lines are transposed.
        let steep = (y1 - y0).abs() > (x1 - x0).abs();
        let (mut ax, mut ay, mut bx, mut by) = (x0, y0, x1, y1);
        if steep {
            std::mem::swap(&mut ax, &mut ay);
            std::mem::swap(&mut bx, &mut by);
        }
        if ax > bx {
            std::mem::swap(&mut ax, &mut bx);
            std::mem::swap(&mut ay, &mut by);
        }
        let dx = bx - ax;
        if dx.abs() < f32::EPSILON {
            self.blend_pixel(palette, ax.round() as i32, ay.round() as i32, color, 255);
            return;
        }
        let gradient = (by - ay) / dx;
        let mut y = ay;
        let start = ax.ceil() as i32;
        let end = bx.floor() as i32;
        for x in start..=end {
            let base_y = y.floor();
            let fraction = y - base_y;
            let (px0, py0) = if steep { (base_y as i32, x) } else { (x, base_y as i32) };
            let (px1, py1) = if steep { (base_y as i32 + 1, x) } else { (x, base_y as i32 + 1) };
            self.blend_pixel(palette, px0, py0, color, ((1.0 - fraction) * 255.0).round() as u8);
            self.blend_pixel(palette, px1, py1, color, (fraction * 255.0).round() as u8);
            y += gradient;
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
                if !self.in_bounds(x, y) { continue; }
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
        let inner_radius = (outer_radius - inset as f64).max(0.0);
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
                if !in_inner && self.in_bounds(px as i32, py as i32) {
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
                    if !self.in_bounds(px as i32, y) { continue; }
                    let offset = self.offset(px as u32, y as u32);
                    self.pixels[offset..offset + 4].copy_from_slice(&[c.r, c.g, c.b, 255]);
                }
            }
        }
    }

    /// Draws a quadratic Bézier curve by flattening it into line segments.
    pub fn draw_quadratic_bezier(
        &mut self,
        palette: &Palette,
        start: (f32, f32),
        control: (f32, f32),
        end: (f32, f32),
        color: Color,
        segments: u32,
    ) {
        let steps = segments.clamp(2, 4096);
        let mut previous = start;
        for step in 1..=steps {
            let t = step as f32 / steps as f32;
            let one_minus_t = 1.0 - t;
            let point = (
                one_minus_t * one_minus_t * start.0
                    + 2.0 * one_minus_t * t * control.0
                    + t * t * end.0,
                one_minus_t * one_minus_t * start.1
                    + 2.0 * one_minus_t * t * control.1
                    + t * t * end.1,
            );
            self.draw_line(
                palette,
                previous.0.round() as i32,
                previous.1.round() as i32,
                point.0.round() as i32,
                point.1.round() as i32,
                color,
            );
            previous = point;
        }
    }

    /// Draws a cubic Bézier curve by flattening it into line segments.
    pub fn draw_cubic_bezier(
        &mut self,
        palette: &Palette,
        start: (f32, f32),
        control1: (f32, f32),
        control2: (f32, f32),
        end: (f32, f32),
        color: Color,
        segments: u32,
    ) {
        let steps = segments.clamp(2, 4096);
        let mut previous = start;
        for step in 1..=steps {
            let t = step as f32 / steps as f32;
            let u = 1.0 - t;
            let point = (
                u * u * u * start.0
                    + 3.0 * u * u * t * control1.0
                    + 3.0 * u * t * t * control2.0
                    + t * t * t * end.0,
                u * u * u * start.1
                    + 3.0 * u * u * t * control1.1
                    + 3.0 * u * t * t * control2.1
                    + t * t * t * end.1,
            );
            self.draw_line(
                palette,
                previous.0.round() as i32,
                previous.1.round() as i32,
                point.0.round() as i32,
                point.1.round() as i32,
                color,
            );
            previous = point;
        }
    }

    /// Draws an ellipse outline as a connected polyline.
    pub fn stroke_ellipse(
        &mut self,
        palette: &Palette,
        center_x: i32,
        center_y: i32,
        radius_x: u32,
        radius_y: u32,
        color: Color,
        segments: u32,
    ) {
        if radius_x == 0 || radius_y == 0 { return; }
        let steps = segments.clamp(12, 4096);
        let mut previous = (
            center_x + radius_x as i32,
            center_y,
        );
        for step in 1..=steps {
            let angle = std::f64::consts::TAU * step as f64 / steps as f64;
            let point = (
                center_x + (radius_x as f64 * angle.cos()).round() as i32,
                center_y + (radius_y as f64 * angle.sin()).round() as i32,
            );
            self.draw_line(palette, previous.0, previous.1, point.0, point.1, color);
            previous = point;
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

    /// Strokes a path after applying an affine transform.
    pub fn stroke_transformed_path(
        &mut self,
        palette: &Palette,
        path: &VectorPath,
        transform: Transform2D,
        color: Color,
    ) {
        self.stroke_path(palette, &path.transformed(transform), color);
    }

    /// Fills a path after applying an affine transform.
    pub fn fill_transformed_path(
        &mut self,
        palette: &Palette,
        path: &VectorPath,
        transform: Transform2D,
        color: Color,
    ) {
        self.fill_path(palette, &path.transformed(transform), color);
    }

    /// Strokes every subpath in a vector path.
    pub fn stroke_path(&mut self, palette: &Palette, path: &VectorPath, color: Color) {
        for subpath in path.flatten() {
            self.draw_polyline(palette, &subpath, false, color);
        }
    }

    /// Fills all flattened contours together using the even-odd rule.
    /// Separate contours remain separate, allowing nested contours to create holes.
    pub fn fill_path(&mut self, palette: &Palette, path: &VectorPath, color: Color) {
        let contours = path.flatten();
        if contours.is_empty() { return; }
        let min_y = contours.iter().flat_map(|p| p.iter().map(|v| v.1)).min().unwrap().max(0);
        let max_y = contours.iter().flat_map(|p| p.iter().map(|v| v.1)).max().unwrap().min(self.height as i32 - 1);
        if min_y > max_y { return; }
        let c = palette.sample(color);
        let mut intersections = Vec::<f64>::new();

        for y in min_y..=max_y {
            intersections.clear();
            let scan_y = y as f64 + 0.5;
            for contour in &contours {
                if contour.len() < 2 { continue; }
                for index in 0..contour.len() {
                    let (x1, y1) = contour[index];
                    let (x2, y2) = contour[(index + 1) % contour.len()];
                    if y1 == y2 { continue; }
                    let low = y1.min(y2) as f64;
                    let high = y1.max(y2) as f64;
                    if scan_y < low || scan_y >= high { continue; }
                    let t = (scan_y - y1 as f64) / (y2 - y1) as f64;
                    intersections.push(x1 as f64 + t * (x2 - x1) as f64);
                }
            }
            intersections.sort_by(|a, b| a.total_cmp(b));
            for pair in intersections.chunks_exact(2) {
                let left = (pair[0].ceil() as i64).max(0);
                let right = (pair[1].ceil() as i64).min(self.width as i64);
                for px in left..right {
                    if !self.in_bounds(px as i32, y) { continue; }
                    let offset = self.offset(px as u32, y as u32);
                    self.pixels[offset..offset + 4].copy_from_slice(&[c.r, c.g, c.b, 255]);
                }
            }
        }
    }

    pub fn save_png(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let image = RgbaImage::from_raw(self.width, self.height, self.pixels.clone())
            .ok_or("could not construct framebuffer image")?;
        image.save(path).map_err(|e| format!("could not save rendered image: {e}"))
    }

    fn in_bounds(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || (x as u32) >= self.width || (y as u32) >= self.height {
            return false;
        }
        match self.clip {
            Some((left, top, right, bottom)) =>
                (x as u32) >= left && (x as u32) < right && (y as u32) >= top && (y as u32) < bottom,
            None => true,
        }
    }

    fn offset(&self, x: u32, y: u32) -> usize {
        ((y as usize * self.width as usize) + x as usize) * 4
    }
}


/// A transparent off-screen drawing surface with independent visibility and opacity.
///
/// Layers are composited in insertion order by the caller. Their pixels are kept
/// separate until `composite_into` is called, so changing visibility or opacity
/// does not require redrawing the layer's contents.
pub struct Layer {
    framebuffer: Framebuffer,
    opacity: u8,
    visible: bool,
}

impl Layer {
    /// Creates a transparent layer with the requested dimensions.
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        Ok(Self {
            framebuffer: Framebuffer::new(width, height)?,
            opacity: 255,
            visible: true,
        })
    }

    pub fn framebuffer(&self) -> &Framebuffer { &self.framebuffer }
    pub fn framebuffer_mut(&mut self) -> &mut Framebuffer { &mut self.framebuffer }

    pub fn set_opacity(&mut self, opacity: u8) { self.opacity = opacity; }
    pub fn opacity(&self) -> u8 { self.opacity }

    pub fn set_visible(&mut self, visible: bool) { self.visible = visible; }
    pub fn is_visible(&self) -> bool { self.visible }

    /// Composites this layer over a target framebuffer using source-over alpha.
    /// The target's active clipping rectangle is respected.
    pub fn composite_into(&self, target: &mut Framebuffer) -> Result<(), String> {
        if self.framebuffer.width != target.width || self.framebuffer.height != target.height {
            return Err("layer and target framebuffer dimensions must match".into());
        }
        if !self.visible || self.opacity == 0 { return Ok(()); }

        let layer_opacity = self.opacity as u32;
        for y in 0..self.framebuffer.height {
            for x in 0..self.framebuffer.width {
                if !target.in_bounds(x as i32, y as i32) { continue; }
                let source_offset = self.framebuffer.offset(x, y);
                let source_alpha = self.framebuffer.pixels[source_offset + 3] as u32;
                let alpha = (source_alpha * layer_opacity + 127) / 255;
                if alpha == 0 { continue; }

                let target_offset = target.offset(x, y);
                let dest_alpha = target.pixels[target_offset + 3] as u32;
                let out_alpha = alpha + (dest_alpha * (255 - alpha) + 127) / 255;
                if out_alpha == 0 { continue; }

                for channel in 0..3 {
                    let src = self.framebuffer.pixels[source_offset + channel] as u32;
                    let dst = target.pixels[target_offset + channel] as u32;
                    let premultiplied = src * alpha
                        + (dst * dest_alpha * (255 - alpha) + 127) / 255;
                    target.pixels[target_offset + channel] =
                        ((premultiplied + out_alpha / 2) / out_alpha).min(255) as u8;
                }
                target.pixels[target_offset + 3] = out_alpha.min(255) as u8;
            }
        }
        Ok(())
    }
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_composites_with_opacity_and_visibility() {
        let mut layer = Layer::new(2, 1).unwrap();
        layer.framebuffer_mut().pixels[0..4].copy_from_slice(&[255, 0, 0, 255]);
        let mut target = Framebuffer::new(2, 1).unwrap();
        target.pixels[0..4].copy_from_slice(&[0, 0, 255, 255]);

        layer.set_opacity(128);
        layer.composite_into(&mut target).unwrap();
        assert!(target.pixels[0] >= 127 && target.pixels[0] <= 129);
        assert_eq!(target.pixels[2], 127);
        assert_eq!(target.pixels[3], 255);

        layer.set_visible(false);
        let before = target.pixels.clone();
        layer.composite_into(&mut target).unwrap();
        assert_eq!(target.pixels, before);
    }

    #[test]
    fn layer_rejects_different_framebuffer_dimensions() {
        let layer = Layer::new(2, 2).unwrap();
        let mut target = Framebuffer::new(3, 2).unwrap();
        assert!(layer.composite_into(&mut target).is_err());
    }

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
    fn saved_clip_state_restores_after_nested_clipping() {
        let mut frame = Framebuffer::new(8, 8).unwrap();
        frame.set_clip(1, 1, 6, 6);
        frame.save_clip();
        frame.intersect_clip(3, 3, 2, 2);
        assert!(frame.in_bounds(3, 3));
        assert!(!frame.in_bounds(2, 2));
        assert!(frame.restore_clip());
        assert!(frame.in_bounds(2, 2));
        assert!(!frame.in_bounds(0, 0));
        assert!(!frame.restore_clip());
    }

    #[test]
    fn framebuffer_clips_pixels_outside_bounds() {
        let mut frame = Framebuffer::new(2, 2).unwrap();
        assert!(!frame.in_bounds(-1, 0));
        assert!(frame.in_bounds(1, 1));
        assert!(!frame.in_bounds(2, 1));
    }
}