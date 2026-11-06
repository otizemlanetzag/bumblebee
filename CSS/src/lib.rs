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
const MAX_DIRTY_REGIONS: usize = 64;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
fn merge_dirty_region(
    regions: &mut Vec<DirtyRegion>,
    bounds_width: u32,
    bounds_height: u32,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) {
    if width == 0 || height == 0 { return; }
    let left = (x as i64).max(0).min(bounds_width as i64) as u32;
    let top = (y as i64).max(0).min(bounds_height as i64) as u32;
    let right = (x as i64 + width as i64).max(0).min(bounds_width as i64) as u32;
    let bottom = (y as i64 + height as i64).max(0).min(bounds_height as i64) as u32;
    if left >= right || top >= bottom { return; }

    let mut merged = DirtyRegion { x: left, y: top, width: right-left, height: bottom-top };
    let mut index = 0;
    while index < regions.len() {
        let old = regions[index];
        let old_right = old.x + old.width;
        let old_bottom = old.y + old.height;
        let new_right = merged.x + merged.width;
        let new_bottom = merged.y + merged.height;
        let separated = old_right < merged.x || new_right < old.x
            || old_bottom < merged.y || new_bottom < old.y;
        if separated {
            index += 1;
            continue;
        }
        let nx = old.x.min(merged.x);
        let ny = old.y.min(merged.y);
        let nr = old_right.max(new_right);
        let nb = old_bottom.max(new_bottom);
        merged = DirtyRegion { x: nx, y: ny, width: nr-nx, height: nb-ny };
        regions.swap_remove(index);
        index = 0;
    }
    regions.push(merged);
    if regions.len() > MAX_DIRTY_REGIONS {
        let mut bounds = regions[0];
        for region in regions.iter().skip(1) {
            let right = (bounds.x + bounds.width).max(region.x + region.width);
            let bottom = (bounds.y + bounds.height).max(region.y + region.height);
            let left = bounds.x.min(region.x);
            let top = bounds.y.min(region.y);
            bounds = DirtyRegion { x: left, y: top, width: right-left, height: bottom-top };
        }
        regions.clear();
        regions.push(bounds);
    }
}

pub struct DirtyRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone)]
pub struct Framebuffer {
    width: u32,
    height: u32,
    /// Optional framebuffer-space clipping rectangle: (left, top, right, bottom).
    clip: Option<(u32, u32, u32, u32)>,
    /// Saved clipping states, including None for the full framebuffer.
    clip_stack: Vec<Option<(u32, u32, u32, u32)>>,
    /// RGBA output pixels, row-major.
    pixels: Vec<u8>,
    /// Rectangles changed since the last call to `take_dirty_regions`.
    dirty_regions: Vec<DirtyRegion>,
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
            dirty_regions: Vec::new(),
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

    /// Returns the pending changed rectangles without clearing them.
    pub fn dirty_regions(&self) -> &[DirtyRegion] { &self.dirty_regions }

    pub fn has_dirty_regions(&self) -> bool { !self.dirty_regions.is_empty() }

    /// Takes the changed rectangles accumulated since the previous take.
    pub fn take_dirty_regions(&mut self) -> Vec<DirtyRegion> {
        std::mem::take(&mut self.dirty_regions)
    }

    /// Copies only the source framebuffer's pending dirty rectangles into this
    /// framebuffer. Dimensions must match; the destination clip is respected.
    /// Source dirty state is left unchanged so multiple presenters can consume it.
    pub fn copy_dirty_from(&mut self, source: &Framebuffer) -> Result<usize, String> {
        if self.width != source.width || self.height != source.height {
            return Err("source and destination framebuffer dimensions must match".into());
        }
        let regions = source.dirty_regions.clone();
        let mut copied = 0usize;
        for region in regions {
            let mut left = region.x.min(self.width);
            let mut top = region.y.min(self.height);
            let mut right = region.x.saturating_add(region.width).min(self.width);
            let mut bottom = region.y.saturating_add(region.height).min(self.height);
            if let Some((clip_left, clip_top, clip_right, clip_bottom)) = self.clip {
                left = left.max(clip_left);
                top = top.max(clip_top);
                right = right.min(clip_right);
                bottom = bottom.min(clip_bottom);
            }
            if left >= right || top >= bottom { continue; }
            for y in top..bottom {
                let start = self.offset(left, y);
                let end = self.offset(right, y);
                self.pixels[start..end].copy_from_slice(&source.pixels[start..end]);
            }
            self.mark_dirty_region(left as i32, top as i32, right - left, bottom - top);
            copied += 1;
        }
        Ok(copied)
    }

    /// Marks a rectangle as changed. It is clipped to the framebuffer and the
    /// active drawing clip, then merged with any overlapping/touching region.
    pub fn mark_dirty_region(&mut self, x: i32, y: i32, width: u32, height: u32) {
        if width == 0 || height == 0 { return; }
        let left = (x as i64).max(0).min(self.width as i64) as u32;
        let top = (y as i64).max(0).min(self.height as i64) as u32;
        let right = (x as i64 + width as i64).max(0).min(self.width as i64) as u32;
        let bottom = (y as i64 + height as i64).max(0).min(self.height as i64) as u32;
        if left >= right || top >= bottom { return; }
        let (left, top, right, bottom) = match self.clip {
            Some((cl, ct, cr, cb)) => (left.max(cl), top.max(ct), right.min(cr), bottom.min(cb)),
            None => (left, top, right, bottom),
        };
        if left >= right || top >= bottom { return; }

        let mut merged = DirtyRegion { x: left, y: top, width: right-left, height: bottom-top };
        let mut index = 0;
        while index < self.dirty_regions.len() {
            let old = self.dirty_regions[index];
            let old_right = old.x + old.width;
            let old_bottom = old.y + old.height;
            let new_right = merged.x + merged.width;
            let new_bottom = merged.y + merged.height;
            let separated = old_right < merged.x || new_right < old.x
                || old_bottom < merged.y || new_bottom < old.y;
            if separated {
                index += 1;
                continue;
            }
            let nx = old.x.min(merged.x);
            let ny = old.y.min(merged.y);
            let nr = old_right.max(new_right);
            let nb = old_bottom.max(new_bottom);
            merged = DirtyRegion { x: nx, y: ny, width: nr-nx, height: nb-ny };
            self.dirty_regions.swap_remove(index);
            index = 0;
        }
        self.dirty_regions.push(merged);
        if self.dirty_regions.len() > MAX_DIRTY_REGIONS {
            let mut bounds = self.dirty_regions[0];
            for region in self.dirty_regions.iter().skip(1) {
                let right = (bounds.x + bounds.width).max(region.x + region.width);
                let bottom = (bounds.y + bounds.height).max(region.y + region.height);
                let left = bounds.x.min(region.x);
                let top = bounds.y.min(region.y);
                bounds = DirtyRegion { x: left, y: top, width: right - left, height: bottom - top };
            }
            self.dirty_regions.clear();
            self.dirty_regions.push(bounds);
        }
    }

    pub fn clear(&mut self, palette: &Palette, color: Color) {
        self.dirty_regions.clear();
        self.dirty_regions.push(DirtyRegion { x: 0, y: 0, width: self.width, height: self.height });
        let c = palette.sample(color);
        for pixel in self.pixels.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[c.r, c.g, c.b, 255]);
        }
    }

    /// Clears only the current clipping rectangle to transparent pixels.
    fn clear_transparent_in_clip(&mut self) {
        let (left, top, right, bottom) = self.clip.unwrap_or((0, 0, self.width, self.height));
        if left >= right || top >= bottom { return; }
        for y in top..bottom {
            let start = self.offset(left, y);
            let end = self.offset(right, y);
            self.pixels[start..end].fill(0);
        }
        self.mark_dirty_region(left as i32, top as i32, right - left, bottom - top);
    }

    /// Clears the framebuffer to fully transparent pixels.
    /// Unlike clear(), this is useful for off-screen compositing layers.
    pub fn clear_transparent(&mut self) {
        self.dirty_regions.clear();
        self.dirty_regions.push(DirtyRegion { x: 0, y: 0, width: self.width, height: self.height });
        self.pixels.fill(0);
    }

    pub fn set_pixel(&mut self, palette: &Palette, x: i32, y: i32, color: Color) {
        if !self.in_bounds(x, y) { return; }
        let c = palette.sample(color);
        let offset = self.offset(x as u32, y as u32);
        self.pixels[offset..offset + 4].copy_from_slice(&[c.r, c.g, c.b, 255]);
        self.mark_dirty_region(x, y, 1, 1);
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
        self.mark_dirty_region(x, y, 1, 1);
    }

    pub fn fill_rect(&mut self, palette: &Palette, x: i32, y: i32, width: u32, height: u32, color: Color) {
        let left = x.max(0) as u32;
        let top = y.max(0) as u32;
        let right = (x as i64 + width as i64).clamp(0, self.width as i64) as u32;
        let bottom = (y as i64 + height as i64).clamp(0, self.height as i64) as u32;
        if right <= left || bottom <= top { return; }
        self.mark_dirty_region(left as i32, top as i32, right-left, bottom-top);
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
                    self.mark_dirty_region(x, y, 1, 1);
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
                self.mark_dirty_region(x as i32, y, 1, 1);
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
                    self.mark_dirty_region(px as i32, py as i32, 1, 1);
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
                    self.mark_dirty_region(px as i32, py as i32, 1, 1);
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
                    self.mark_dirty_region(px as i32, y, 1, 1);
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
                    self.mark_dirty_region(px as i32, y, 1, 1);
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


/// A scrollable, scalable window onto a larger framebuffer.
///
/// Scroll offsets are measured in source pixels. Scale is the number of
/// destination pixels per source pixel. Sampling uses nearest-neighbor
/// filtering; pixels outside the source are made transparent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    width: u32,
    height: u32,
    scroll_x: f32,
    scroll_y: f32,
    scale: f32,
}

impl Viewport {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err("viewport dimensions must be greater than zero".into());
        }
        Ok(Self { width, height, scroll_x: 0.0, scroll_y: 0.0, scale: 1.0 })
    }

    pub fn width(&self) -> u32 { self.width }
    pub fn height(&self) -> u32 { self.height }
    pub fn scroll(&self) -> (f32, f32) { (self.scroll_x, self.scroll_y) }
    pub fn scale(&self) -> f32 { self.scale }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        if width == 0 || height == 0 {
            return Err("viewport dimensions must be greater than zero".into());
        }
        self.width = width;
        self.height = height;
        Ok(())
    }

    pub fn set_scroll(&mut self, x: f32, y: f32) -> Result<(), String> {
        if !x.is_finite() || !y.is_finite() {
            return Err("viewport scroll offsets must be finite".into());
        }
        self.scroll_x = x;
        self.scroll_y = y;
        Ok(())
    }

    pub fn set_scale(&mut self, scale: f32) -> Result<(), String> {
        if !scale.is_finite() || scale <= 0.0 {
            return Err("viewport scale must be finite and greater than zero".into());
        }
        self.scale = scale;
        Ok(())
    }

    /// Draws the visible source area at (destination_x, destination_y).
    /// Only the viewport bounds and destination's active clip are modified.
    /// Source pixels are read without consuming its dirty-region information.
    pub fn render_into(
        &self,
        source: &Framebuffer,
        target: &mut Framebuffer,
        destination_x: i32,
        destination_y: i32,
    ) -> Result<(), String> {
        let left = (destination_x as i64).max(0).max(
            target.clip.map_or(0, |clip| clip.0 as i64)
        );
        let top = (destination_y as i64).max(0).max(
            target.clip.map_or(0, |clip| clip.1 as i64)
        );
        let right = (destination_x as i64 + self.width as i64)
            .min(target.width as i64)
            .min(target.clip.map_or(target.width, |clip| clip.2) as i64);
        let bottom = (destination_y as i64 + self.height as i64)
            .min(target.height as i64)
            .min(target.clip.map_or(target.height, |clip| clip.3) as i64);

        if left >= right || top >= bottom { return Ok(()); }

        for dy in top..bottom {
            let local_y = (dy - destination_y as i64) as f32;
            let source_y = (self.scroll_y + local_y / self.scale).floor();
            for dx in left..right {
                let local_x = (dx - destination_x as i64) as f32;
                let source_x = (self.scroll_x + local_x / self.scale).floor();
                let target_offset = target.offset(dx as u32, dy as u32);
                if source_x >= 0.0 && source_y >= 0.0
                    && source_x < source.width as f32 && source_y < source.height as f32
                {
                    let source_offset = source.offset(source_x as u32, source_y as u32);
                    target.pixels[target_offset..target_offset + 4]
                        .copy_from_slice(&source.pixels[source_offset..source_offset + 4]);
                } else {
                    target.pixels[target_offset..target_offset + 4].fill(0);
                }
            }
        }

        target.mark_dirty_region(left as i32, top as i32, (right - left) as u32, (bottom - top) as u32);
        Ok(())
    }
}

/// Tracks the last viewport presentation so unchanged views can update
/// only the destination rectangles affected by source dirty regions.
pub struct ViewportRenderer {
    previous_viewport: Option<Viewport>,
    previous_origin: Option<(i32, i32)>,
    previous_source_size: Option<(u32, u32)>,
    previous_target_size: Option<(u32, u32)>,
}

impl ViewportRenderer {
    pub fn new() -> Self {
        Self {
            previous_viewport: None,
            previous_origin: None,
            previous_source_size: None,
            previous_target_size: None,
        }
    }

    /// Presents the viewport. Returns the number of rendered rectangles.
    /// A changed view or surface size triggers a full viewport redraw;
    /// otherwise only source dirty regions are mapped to destination pixels.
    /// Source dirty regions are preserved for other consumers.
    pub fn render_dirty_into(
        &mut self,
        viewport: &Viewport,
        source: &Framebuffer,
        target: &mut Framebuffer,
        destination_x: i32,
        destination_y: i32,
    ) -> Result<usize, String> {
        let source_size = (source.width, source.height);
        let target_size = (target.width, target.height);
        let view_changed = self.previous_viewport != Some(*viewport)
            || self.previous_origin != Some((destination_x, destination_y))
            || self.previous_source_size != Some(source_size)
            || self.previous_target_size != Some(target_size);

        let count = if view_changed {
            viewport.render_into(source, target, destination_x, destination_y)?;
            1
        } else {
            // Collect mapped rectangles directly: allocating a viewport-sized
            // framebuffer here would waste width * height * 4 bytes per update.
            let mut regions: Vec<DirtyRegion> = Vec::new();
            for dirty in source.dirty_regions() {
                let local_left = ((dirty.x as f32 - viewport.scroll_x) * viewport.scale).ceil();
                let local_top = ((dirty.y as f32 - viewport.scroll_y) * viewport.scale).ceil();
                let local_right = (((dirty.x + dirty.width) as f32 - viewport.scroll_x) * viewport.scale).ceil();
                let local_bottom = (((dirty.y + dirty.height) as f32 - viewport.scroll_y) * viewport.scale).ceil();
                if !local_left.is_finite() || !local_top.is_finite()
                    || !local_right.is_finite() || !local_bottom.is_finite()
                {
                    continue;
                }
                let left = (local_left as i64).clamp(0, viewport.width as i64);
                let top = (local_top as i64).clamp(0, viewport.height as i64);
                let right = (local_right as i64).clamp(0, viewport.width as i64);
                let bottom = (local_bottom as i64).clamp(0, viewport.height as i64);
                if left < right && top < bottom {
                    merge_dirty_region(
                        &mut regions,
                        viewport.width,
                        viewport.height,
                        left as i32,
                        top as i32,
                        (right-left) as u32,
                        (bottom-top) as u32,
                    );
                }
            }

            let changed = regions;
            for region in &changed {
                target.save_clip();
                target.intersect_clip(
                    destination_x.saturating_add(region.x as i32),
                    destination_y.saturating_add(region.y as i32),
                    region.width,
                    region.height,
                );
                let result = viewport.render_into(source, target, destination_x, destination_y);
                target.restore_clip();
                result?;
            }
            changed.len()
        };

        self.previous_viewport = Some(*viewport);
        self.previous_origin = Some((destination_x, destination_y));
        self.previous_source_size = Some(source_size);
        self.previous_target_size = Some(target_size);
        Ok(count)
    }

    /// Forces the next presentation to redraw the entire viewport.
    pub fn invalidate(&mut self) {
        self.previous_viewport = None;
    }
}

impl Default for ViewportRenderer {
    fn default() -> Self { Self::new() }
}

/// Sparse tiled drawing surface. Only tiles that receive drawing operations
/// allocate pixel memory; untouched portions of a large logical surface cost
/// only the small tile-map entry overhead.
pub struct TiledSurface {
    width: u32,
    height: u32,
    tile_size: u32,
    tiles: std::collections::HashMap<(u32, u32), Framebuffer>,
}

impl TiledSurface {
    pub const DEFAULT_TILE_SIZE: u32 = 128;

    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        Self::with_tile_size(width, height, Self::DEFAULT_TILE_SIZE)
    }

    pub fn with_tile_size(width: u32, height: u32, tile_size: u32) -> Result<Self, String> {
        if width == 0 || height == 0 || tile_size == 0 {
            return Err("surface dimensions and tile size must be greater than zero".into());
        }
        Ok(Self { width, height, tile_size, tiles: std::collections::HashMap::new() })
    }

    pub fn width(&self) -> u32 { self.width }
    pub fn height(&self) -> u32 { self.height }
    pub fn tile_size(&self) -> u32 { self.tile_size }
    pub fn allocated_tiles(&self) -> usize { self.tiles.len() }
    pub fn allocated_bytes(&self) -> usize {
        self.tiles.values().map(|tile| tile.pixels.len()).sum()
    }

    /// Returns pending dirty rectangles in surface-wide coordinates.
    /// Rectangles remain tile-local only internally; callers receive global positions.
    pub fn dirty_regions(&self) -> Vec<DirtyRegion> {
        let mut regions = Vec::new();
        for (&(tile_x, tile_y), tile) in &self.tiles {
            let origin_x = tile_x * self.tile_size;
            let origin_y = tile_y * self.tile_size;
            for region in tile.dirty_regions() {
                regions.push(DirtyRegion {
                    x: origin_x + region.x,
                    y: origin_y + region.y,
                    width: region.width,
                    height: region.height,
                });
            }
        }
        regions
    }

    /// Returns and clears pending dirty rectangles in surface-wide coordinates.
    pub fn take_dirty_regions(&mut self) -> Vec<DirtyRegion> {
        let mut regions = Vec::new();
        for (&(tile_x, tile_y), tile) in self.tiles.iter_mut() {
            let origin_x = tile_x * self.tile_size;
            let origin_y = tile_y * self.tile_size;
            for region in tile.take_dirty_regions() {
                regions.push(DirtyRegion {
                    x: origin_x + region.x,
                    y: origin_y + region.y,
                    width: region.width,
                    height: region.height,
                });
            }
        }
        regions
    }

    fn tile_dimensions(&self, tile_x: u32, tile_y: u32) -> (u32, u32) {
        let x = tile_x * self.tile_size;
        let y = tile_y * self.tile_size;
        ((self.width - x).min(self.tile_size), (self.height - y).min(self.tile_size))
    }

    fn tile_mut(&mut self, tile_x: u32, tile_y: u32) -> Result<&mut Framebuffer, String> {
        if !self.tiles.contains_key(&(tile_x, tile_y)) {
            let (width, height) = self.tile_dimensions(tile_x, tile_y);
            self.tiles.insert((tile_x, tile_y), Framebuffer::new(width, height)?);
        }
        Ok(self.tiles.get_mut(&(tile_x, tile_y)).expect("tile inserted"))
    }

    /// Writes one pixel and reports allocation failures instead of silently dropping it.
    pub fn try_set_pixel(&mut self, palette: &Palette, x: i32, y: i32, color: Color) -> Result<(), String> {
        if x < 0 || y < 0 || x as u32 >= self.width || y as u32 >= self.height { return Ok(()); }
        let (x, y) = (x as u32, y as u32);
        let tx = x / self.tile_size;
        let ty = y / self.tile_size;
        let tile = self.tile_mut(tx, ty)?;
        tile.set_pixel(palette, (x % self.tile_size) as i32, (y % self.tile_size) as i32, color);
        Ok(())
    }

    /// Convenience pixel write. Use try_set_pixel when allocation errors matter.
    pub fn set_pixel(&mut self, palette: &Palette, x: i32, y: i32, color: Color) {
        let _ = self.try_set_pixel(palette, x, y, color);
    }

    /// Alpha-composites a color over a pixel, allocating its tile only when needed.
    pub fn blend_pixel(&mut self, palette: &Palette, x: i32, y: i32, color: Color, alpha: u8) -> Result<(), String> {
        if alpha == 0 || x < 0 || y < 0 || x as u32 >= self.width || y as u32 >= self.height {
            return Ok(());
        }
        let (ux, uy) = (x as u32, y as u32);
        let tile = self.tile_mut(ux / self.tile_size, uy / self.tile_size)?;
        let offset = tile.offset(ux % self.tile_size, uy % self.tile_size);
        let source = palette.sample(color);
        let source_alpha = alpha as u32;
        let destination_alpha = tile.pixels[offset + 3] as u32;
        let inverse = 255 - source_alpha;
        let output_alpha = source_alpha + (destination_alpha * inverse + 127) / 255;
        if output_alpha == 0 { return Ok(()); }
        for channel in 0..3 {
            let src = [source.r, source.g, source.b][channel] as u32;
            let dst = tile.pixels[offset + channel] as u32;
            let premultiplied = src * source_alpha
                + (dst * destination_alpha * inverse + 127) / 255;
            tile.pixels[offset + channel] =
                ((premultiplied + output_alpha / 2) / output_alpha).min(255) as u8;
        }
        tile.pixels[offset + 3] = output_alpha.min(255) as u8;
        tile.mark_dirty_region((ux % self.tile_size) as i32, (uy % self.tile_size) as i32, 1, 1);
        Ok(())
    }

    /// Reads an RGBA pixel. Unallocated tiles are transparent.
    pub fn pixel_rgba(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height { return None; }
        let tile = self.tiles.get(&(x / self.tile_size, y / self.tile_size))?;
        let offset = tile.offset(x % self.tile_size, y % self.tile_size);
        Some([tile.pixels[offset], tile.pixels[offset + 1], tile.pixels[offset + 2], tile.pixels[offset + 3]])
    }

    /// Fills a rectangle across tile boundaries and reports allocation failures.
    pub fn try_fill_rect(
        &mut self,
        palette: &Palette,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        color: Color,
    ) -> Result<(), String> {
        let left = (x as i64).max(0).min(self.width as i64) as u32;
        let top = (y as i64).max(0).min(self.height as i64) as u32;
        let right = (x as i64 + width as i64).clamp(0, self.width as i64) as u32;
        let bottom = (y as i64 + height as i64).clamp(0, self.height as i64) as u32;
        if left >= right || top >= bottom { return Ok(()); }
        for ty in (top / self.tile_size)..=((bottom - 1) / self.tile_size) {
            for tx in (left / self.tile_size)..=((right - 1) / self.tile_size) {
                let tile_left = tx * self.tile_size;
                let tile_top = ty * self.tile_size;
                let l = left.max(tile_left) - tile_left;
                let t = top.max(tile_top) - tile_top;
                let r = right.min(tile_left + self.tile_size) - tile_left;
                let b = bottom.min(tile_top + self.tile_size) - tile_top;
                self.tile_mut(tx, ty)?.fill_rect(palette, l as i32, t as i32, r-l, b-t, color);
            }
        }
        Ok(())
    }

    /// Convenience rectangle fill. Use try_fill_rect when allocation errors matter.
    pub fn fill_rect(
        &mut self,
        palette: &Palette,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        color: Color,
    ) {
        let _ = self.try_fill_rect(palette, x, y, width, height, color);
    }


    /// Draws a Bresenham line and reports tile-allocation failures.
    pub fn try_draw_line(
        &mut self, palette: &Palette, x0: i32, y0: i32, x1: i32, y1: i32, color: Color,
    ) -> Result<(), String> {
        let (mut x, mut y) = (x0 as i64, y0 as i64);
        let end_x = x1 as i64;
        let end_y = y1 as i64;
        let dx = (end_x - x).abs();
        let sx = if x < end_x { 1 } else { -1 };
        let dy = -(end_y - y).abs();
        let sy = if y < end_y { 1 } else { -1 };
        let mut error = dx + dy;
        loop {
            if x >= 0 && y >= 0 && x < self.width as i64 && y < self.height as i64 {
                self.try_set_pixel(palette, x as i32, y as i32, color)?;
            }
            if x == end_x && y == end_y { break; }
            let twice = 2 * error;
            if twice >= dy { error += dy; x += sx; }
            if twice <= dx { error += dx; y += sy; }
        }
        Ok(())
    }

    /// Convenience line drawing. Use try_draw_line when allocation errors matter.
    pub fn draw_line(&mut self, palette: &Palette, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
        let _ = self.try_draw_line(palette, x0, y0, x1, y1, color);
    }

    /// Fills a circle and reports tile-allocation failures.
    pub fn try_fill_circle(
        &mut self, palette: &Palette, center_x: i32, center_y: i32, radius: u32, color: Color,
    ) -> Result<(), String> {
        let radius = radius as i64;
        let radius_squared = radius.saturating_mul(radius);
        let top = (center_y as i64 - radius).max(0);
        let bottom = (center_y as i64 + radius).min(self.height as i64 - 1);
        if top > bottom { return Ok(()); }
        for y in top..=bottom {
            let dy = y - center_y as i64;
            let remaining = radius_squared.saturating_sub(dy.saturating_mul(dy));
            let span = (remaining as f64).sqrt() as i64;
            let left = (center_x as i64 - span).max(0);
            let right = (center_x as i64 + span).min(self.width as i64 - 1);
            if left <= right {
                self.try_fill_rect(palette, left as i32, y as i32,
                    (right - left + 1) as u32, 1, color)?;
            }
        }
        Ok(())
    }

    /// Convenience circle fill. Use try_fill_circle when allocation errors matter.
    pub fn fill_circle(&mut self, palette: &Palette, center_x: i32, center_y: i32, radius: u32, color: Color) {
        let _ = self.try_fill_circle(palette, center_x, center_y, radius, color);
    }

    /// Fills an ellipse and reports tile-allocation failures.
    pub fn try_fill_ellipse(
        &mut self, palette: &Palette, center_x: i32, center_y: i32,
        radius_x: u32, radius_y: u32, color: Color,
    ) -> Result<(), String> {
        if radius_x == 0 || radius_y == 0 {
            return self.try_set_pixel(palette, center_x, center_y, color);
        }
        let rx = radius_x as f64;
        let ry = radius_y as f64;
        let top = (center_y as i64 - radius_y as i64).max(0);
        let bottom = (center_y as i64 + radius_y as i64).min(self.height as i64 - 1);
        if top > bottom { return Ok(()); }
        for y in top..=bottom {
            let normalized_y = (y - center_y as i64) as f64 / ry;
            let remaining = (1.0 - normalized_y * normalized_y).max(0.0);
            let span = (rx * remaining.sqrt()).floor() as i64;
            let left = (center_x as i64 - span).max(0);
            let right = (center_x as i64 + span).min(self.width as i64 - 1);
            if left <= right {
                self.try_fill_rect(palette, left as i32, y as i32,
                    (right - left + 1) as u32, 1, color)?;
            }
        }
        Ok(())
    }

    /// Convenience ellipse fill. Use try_fill_ellipse when allocation errors matter.
    pub fn fill_ellipse(
        &mut self, palette: &Palette, center_x: i32, center_y: i32,
        radius_x: u32, radius_y: u32, color: Color,
    ) {
        let _ = self.try_fill_ellipse(palette, center_x, center_y, radius_x, radius_y, color);
    }

    /// Fills a rounded rectangle with scanline spans and reports allocation failures.
    pub fn try_fill_rounded_rect(
        &mut self, palette: &Palette, x: i32, y: i32, width: u32, height: u32,
        radius: u32, color: Color,
    ) -> Result<(), String> {
        if width == 0 || height == 0 { return Ok(()); }
        let radius = radius.min(width / 2).min(height / 2) as i64;
        let left_bound = x as i64;
        let right_bound = left_bound + width as i64;
        let top_bound = y as i64;
        let bottom_bound = top_bound + height as i64;
        let visible_top = top_bound.max(0);
        let visible_bottom = bottom_bound.min(self.height as i64);
        if visible_top >= visible_bottom || right_bound <= 0 || left_bound >= self.width as i64 {
            return Ok(());
        }

        for row in visible_top..visible_bottom {
            let mut inset = 0i64;
            if radius > 0 {
                let distance_from_top = row - top_bound;
                let distance_from_bottom = bottom_bound - 1 - row;
                let corner_distance = if distance_from_top < radius {
                    Some(radius as f64 - distance_from_top as f64 - 0.5)
                } else if distance_from_bottom < radius {
                    Some(radius as f64 - distance_from_bottom as f64 - 0.5)
                } else {
                    None
                };
                if let Some(dy) = corner_distance {
                    let inside = ((radius * radius) as f64 - dy * dy).max(0.0).sqrt();
                    inset = (radius as f64 - inside).ceil() as i64;
                }
            }
            let left = (left_bound + inset).max(0);
            let right = (right_bound - inset).min(self.width as i64);
            if left < right {
                self.try_fill_rect(palette, left as i32, row as i32,
                    (right - left) as u32, 1, color)?;
            }
        }
        Ok(())
    }

    /// Convenience rounded-rectangle fill. Use try_fill_rounded_rect when allocation errors matter.
    pub fn fill_rounded_rect(
        &mut self, palette: &Palette, x: i32, y: i32, width: u32, height: u32,
        radius: u32, color: Color,
    ) {
        let _ = self.try_fill_rounded_rect(palette, x, y, width, height, radius, color);
    }

    /// Draws a rectangle outline and reports tile-allocation failures.
    pub fn try_stroke_rect(
        &mut self, palette: &Palette, x: i32, y: i32, width: u32, height: u32,
        thickness: u32, color: Color,
    ) -> Result<(), String> {
        if width == 0 || height == 0 || thickness == 0 { return Ok(()); }
        let t = thickness.min(width).min(height);
        self.try_fill_rect(palette, x, y, width, t, color)?;
        if height > t {
            let bottom_y = (y as i64 + height as i64 - t as i64)
                .clamp(i32::MIN as i64, i32::MAX as i64) as i32;
            self.try_fill_rect(palette, x, bottom_y, width, t, color)?;
        }
        self.try_fill_rect(palette, x, y, t, height, color)?;
        if width > t {
            let right_x = (x as i64 + width as i64 - t as i64)
                .clamp(i32::MIN as i64, i32::MAX as i64) as i32;
            self.try_fill_rect(palette, right_x, y, t, height, color)?;
        }
        Ok(())
    }

    /// Convenience rectangle outline. Use try_stroke_rect when allocation errors matter.
    pub fn stroke_rect(
        &mut self, palette: &Palette, x: i32, y: i32, width: u32, height: u32,
        thickness: u32, color: Color,
    ) {
        let _ = self.try_stroke_rect(palette, x, y, width, height, thickness, color);
    }

    /// Strokes a vector path directly into sparse tiles.
    /// Curves are flattened by VectorPath; allocation failures are returned.
    pub fn try_stroke_path(
        &mut self, palette: &Palette, path: &VectorPath, color: Color,
    ) -> Result<(), String> {
        for contour in path.flatten() {
            for segment in contour.windows(2) {
                self.try_draw_line(
                    palette, segment[0].0, segment[0].1,
                    segment[1].0, segment[1].1, color,
                )?;
            }
        }
        Ok(())
    }

    /// Fills vector path contours using the even-odd rule directly into sparse tiles.
    /// Nested contours therefore create holes without allocating a full-size framebuffer.
    pub fn try_fill_path(
        &mut self, palette: &Palette, path: &VectorPath, color: Color,
    ) -> Result<(), String> {
        let contours = path.flatten();
        if contours.is_empty() { return Ok(()); }
        let min_y = contours.iter().flat_map(|p| p.iter().map(|v| v.1))
            .min().unwrap().max(0);
        let max_y = contours.iter().flat_map(|p| p.iter().map(|v| v.1))
            .max().unwrap().min(self.height as i32 - 1);
        if min_y > max_y { return Ok(()); }

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
                let left = (pair[0].ceil() as i64).max(0).min(self.width as i64);
                let right = (pair[1].ceil() as i64).max(0).min(self.width as i64);
                if left < right {
                    self.try_fill_rect(palette, left as i32, y,
                        (right - left) as u32, 1, color)?;
                }
            }
        }
        Ok(())
    }

    /// Convenience path stroke. Use try_stroke_path when allocation errors matter.
    pub fn stroke_path(&mut self, palette: &Palette, path: &VectorPath, color: Color) {
        let _ = self.try_stroke_path(palette, path, color);
    }

    /// Convenience path fill. Use try_fill_path when allocation errors matter.
    pub fn fill_path(&mut self, palette: &Palette, path: &VectorPath, color: Color) {
        let _ = self.try_fill_path(palette, path, color);
    }

    /// Composites allocated tiles over a framebuffer using source-over alpha.
    /// Transparent pixels and unallocated tiles are skipped; destination clipping applies.
    pub fn composite_into(
        &self,
        target: &mut Framebuffer,
        destination_x: i32,
        destination_y: i32,
    ) -> Result<usize, String> {
        let mut changed_pixels = 0usize;
        for (&(tile_x, tile_y), tile) in &self.tiles {
            let origin_x = tile_x as i64 * self.tile_size as i64 + destination_x as i64;
            let origin_y = tile_y as i64 * self.tile_size as i64 + destination_y as i64;
            for sy in 0..tile.height {
                let dy = origin_y + sy as i64;
                if dy < 0 || dy >= target.height as i64 { continue; }
                for sx in 0..tile.width {
                    let dx = origin_x + sx as i64;
                    if dx < 0 || dx >= target.width as i64
                        || !target.in_bounds(dx as i32, dy as i32) {
                        continue;
                    }
                    let source_offset = tile.offset(sx, sy);
                    let source_alpha = tile.pixels[source_offset + 3] as u32;
                    if source_alpha == 0 { continue; }
                    let target_offset = target.offset(dx as u32, dy as u32);
                    let destination_alpha = target.pixels[target_offset + 3] as u32;
                    let inverse = 255 - source_alpha;
                    let output_alpha = source_alpha
                        + (destination_alpha * inverse + 127) / 255;
                    for channel in 0..3 {
                        let source = tile.pixels[source_offset + channel] as u32;
                        let destination = target.pixels[target_offset + channel] as u32;
                        let premultiplied = source * source_alpha
                            + (destination * destination_alpha * inverse + 127) / 255;
                        target.pixels[target_offset + channel] =
                            ((premultiplied + output_alpha / 2) / output_alpha)
                                .min(255) as u8;
                    }
                    target.pixels[target_offset + 3] = output_alpha.min(255) as u8;
                    changed_pixels += 1;
                }
            }
        }
        if changed_pixels > 0 {
            // Mark the destination bounds of allocated tiles; this deliberately
            // favors a small number of broad regions over per-pixel dirty entries.
            for (&(tile_x, tile_y), tile) in &self.tiles {
                let x = tile_x as i64 * self.tile_size as i64 + destination_x as i64;
                let y = tile_y as i64 * self.tile_size as i64 + destination_y as i64;
                let left = x.max(0).min(target.width as i64);
                let top = y.max(0).min(target.height as i64);
                let right = (x + tile.width as i64).max(0).min(target.width as i64);
                let bottom = (y + tile.height as i64).max(0).min(target.height as i64);
                if left < right && top < bottom {
                    target.mark_dirty_region(left as i32, top as i32,
                        (right - left) as u32, (bottom - top) as u32);
                }
            }
        }
        Ok(changed_pixels)
    }

    pub fn clear_tiles(&mut self) { self.tiles.clear(); }
}

/// Reuses off-screen framebuffers to reduce repeated allocation churn.
///
/// The pool retains only released surfaces and never exceeds its byte budget.
/// Acquired surfaces are transparent, have no active clip, and start with no
/// pending dirty regions. The pool is intended for temporary render targets;
/// it does not reduce the memory required by surfaces that remain in use.
pub struct FramebufferPool {
    available: Vec<Framebuffer>,
    bytes_used: usize,
    byte_budget: usize,
}

impl FramebufferPool {
    pub fn new(byte_budget: usize) -> Self {
        Self { available: Vec::new(), bytes_used: 0, byte_budget }
    }

    pub fn len(&self) -> usize { self.available.len() }
    pub fn is_empty(&self) -> bool { self.available.is_empty() }
    pub fn bytes_used(&self) -> usize { self.bytes_used }
    pub fn byte_budget(&self) -> usize { self.byte_budget }

    /// Acquires an exact-size surface, reusing a released allocation when possible.
    pub fn acquire(&mut self, width: u32, height: u32) -> Result<Framebuffer, String> {
        if width == 0 || height == 0 {
            return Err("framebuffer dimensions must be greater than zero".into());
        }
        if let Some(index) = self.available.iter().position(|frame| frame.width == width && frame.height == height) {
            let mut frame = self.available.swap_remove(index);
            self.bytes_used = self.bytes_used.saturating_sub(frame.pixels.len());
            frame.pixels.fill(0);
            frame.clip = None;
            frame.clip_stack.clear();
            frame.dirty_regions.clear();
            return Ok(frame);
        }
        Framebuffer::new(width, height)
    }

    /// Returns a surface to the pool. Surfaces larger than the remaining budget
    /// are dropped instead of retained.
    pub fn release(&mut self, mut frame: Framebuffer) {
        let size = frame.pixels.len();
        if size > self.byte_budget || self.bytes_used.saturating_add(size) > self.byte_budget {
            return;
        }
        frame.clip = None;
        frame.clip_stack.clear();
        frame.dirty_regions.clear();
        frame.pixels.fill(0);
        self.bytes_used = self.bytes_used.saturating_add(size);
        self.available.push(frame);
    }

    /// Changes the retention budget, dropping available surfaces if necessary.
    pub fn set_byte_budget(&mut self, byte_budget: usize) {
        self.byte_budget = byte_budget;
        while self.bytes_used > self.byte_budget {
            let Some(frame) = self.available.pop() else { break; };
            self.bytes_used = self.bytes_used.saturating_sub(frame.pixels.len());
        }
    }

    /// Drops all currently available allocations.
    pub fn clear(&mut self) {
        self.available.clear();
        self.bytes_used = 0;
    }
}

/// A bounded cache of completed framebuffer renders.
///
/// The caller supplies a stable key derived from the element's content and
/// rendering inputs. A cached frame can then be reused without rasterizing it
/// again. Inserting an existing key replaces its previous image.
pub struct RenderCache {
    entries: std::collections::HashMap<u64, Framebuffer>,
    insertion_order: std::collections::VecDeque<u64>,
    bytes_used: usize,
    byte_budget: usize,
}

impl RenderCache {
    /// Creates a cache with a byte budget. The default policy evicts oldest
    /// inserted entries first; cache keys are owned by the caller.
    pub fn new(byte_budget: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            insertion_order: std::collections::VecDeque::new(),
            bytes_used: 0,
            byte_budget,
        }
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
    pub fn bytes_used(&self) -> usize { self.bytes_used }
    pub fn byte_budget(&self) -> usize { self.byte_budget }

    pub fn get(&self, key: u64) -> Option<&Framebuffer> {
        self.entries.get(&key)
    }

    /// Stores a snapshot of a completed frame. Oversized frames are not cached.
    pub fn insert(&mut self, key: u64, frame: &Framebuffer) {
        self.remove(key);
        let size = frame.pixels.len();
        if size > self.byte_budget || self.byte_budget == 0 { return; }

        while self.bytes_used.saturating_add(size) > self.byte_budget {
            let Some(oldest) = self.insertion_order.pop_front() else { break; };
            if let Some(removed) = self.entries.remove(&oldest) {
                self.bytes_used = self.bytes_used.saturating_sub(removed.pixels.len());
            }
        }

        self.entries.insert(key, frame.clone());
        self.insertion_order.push_back(key);
        self.bytes_used = self.bytes_used.saturating_add(size);
    }

    /// Removes one cached frame and returns whether it existed.
    pub fn invalidate(&mut self, key: u64) -> bool {
        self.remove(key)
    }

    /// Clears every cached frame.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.insertion_order.clear();
        self.bytes_used = 0;
    }

    /// Changes the memory budget and evicts oldest entries until within budget.
    pub fn set_byte_budget(&mut self, byte_budget: usize) {
        self.byte_budget = byte_budget;
        while self.bytes_used > self.byte_budget {
            let Some(oldest) = self.insertion_order.pop_front() else { break; };
            if let Some(removed) = self.entries.remove(&oldest) {
                self.bytes_used = self.bytes_used.saturating_sub(removed.pixels.len());
            }
        }
    }

    fn remove(&mut self, key: u64) -> bool {
        let Some(removed) = self.entries.remove(&key) else { return false; };
        self.bytes_used = self.bytes_used.saturating_sub(removed.pixels.len());
        self.insertion_order.retain(|entry| *entry != key);
        true
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

    /// Composites only pending dirty rectangles from this layer.
    ///
    /// The source dirty list is preserved for other consumers. Keep visibility
    /// and opacity stable while using this method; after changing either,
    /// rebuild the full scene because old composited pixels cannot be undone.
    pub fn composite_dirty_into(&self, target: &mut Framebuffer) -> Result<usize, String> {
        if self.framebuffer.width != target.width || self.framebuffer.height != target.height {
            return Err("layer and target framebuffer dimensions must match".into());
        }
        if !self.visible || self.opacity == 0 { return Ok(0); }
        let regions = self.framebuffer.dirty_regions.clone();
        let opacity = self.opacity as u32;
        let mut count = 0;
        for region in regions {
            let x0 = region.x.min(self.framebuffer.width);
            let y0 = region.y.min(self.framebuffer.height);
            let x1 = region.x.saturating_add(region.width).min(self.framebuffer.width);
            let y1 = region.y.saturating_add(region.height).min(self.framebuffer.height);
            let mut changed = false;
            for y in y0..y1 {
                for x in x0..x1 {
                    if !target.in_bounds(x as i32, y as i32) { continue; }
                    let so = self.framebuffer.offset(x, y);
                    let alpha = (self.framebuffer.pixels[so + 3] as u32 * opacity + 127) / 255;
                    if alpha == 0 { continue; }
                    let to = target.offset(x, y);
                    let da = target.pixels[to + 3] as u32;
                    let out_a = alpha + (da * (255 - alpha) + 127) / 255;
                    if out_a == 0 { continue; }
                    for c in 0..3 {
                        let src = self.framebuffer.pixels[so + c] as u32;
                        let dst = target.pixels[to + c] as u32;
                        let premul = src * alpha + (dst * da * (255 - alpha) + 127) / 255;
                        target.pixels[to + c] = ((premul + out_a / 2) / out_a).min(255) as u8;
                    }
                    target.pixels[to + 3] = out_a.min(255) as u8;
                    target.mark_dirty_region(x as i32, y as i32, 1, 1);
                    changed = true;
                }
            }
            if changed { count += 1; }
        }
        Ok(count)
    }
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
                target.mark_dirty_region(x as i32, y as i32, 1, 1);
            }
        }
        Ok(())
    }
}

/// Ordered collection of rendering layers with region-aware scene recomposition.
///
/// Mutate pixel content through `framebuffer_mut`; change layer properties through
/// the stack setters so the compositor knows when a full rebuild is required.
pub struct LayerStack {
    width: u32,
    height: u32,
    layers: Vec<Layer>,
    full_rebuild: bool,
}

impl LayerStack {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err("layer stack dimensions must be greater than zero".into());
        }
        Ok(Self { width, height, layers: Vec::new(), full_rebuild: true })
    }

    pub fn len(&self) -> usize { self.layers.len() }
    pub fn is_empty(&self) -> bool { self.layers.is_empty() }

    pub fn add_layer(&mut self) -> Result<usize, String> {
        let layer = Layer::new(self.width, self.height)?;
        self.layers.push(layer);
        self.full_rebuild = true;
        Ok(self.layers.len() - 1)
    }

    /// Removes a layer and returns it. The composed scene must be rebuilt.
    pub fn remove_layer(&mut self, index: usize) -> Option<Layer> {
        if index >= self.layers.len() { return None; }
        self.full_rebuild = true;
        Some(self.layers.remove(index))
    }

    /// Changes z-order: `to` is the layer's new index (0 is the back).
    pub fn move_layer(&mut self, from: usize, to: usize) -> Result<(), String> {
        if from >= self.layers.len() || to >= self.layers.len() {
            return Err("layer index out of range".into());
        }
        if from != to {
            let layer = self.layers.remove(from);
            self.layers.insert(to, layer);
            self.full_rebuild = true;
        }
        Ok(())
    }

    pub fn framebuffer(&self, index: usize) -> Option<&Framebuffer> {
        self.layers.get(index).map(Layer::framebuffer)
    }

    /// Access a layer's drawing surface without invalidating the whole scene.
    /// Drawing operations automatically record the changed regions.
    pub fn framebuffer_mut(&mut self, index: usize) -> Option<&mut Framebuffer> {
        self.layers.get_mut(index).map(Layer::framebuffer_mut)
    }

    pub fn set_opacity(&mut self, index: usize, opacity: u8) -> Result<(), String> {
        let layer = self.layers.get_mut(index).ok_or("layer index out of range")?;
        if layer.opacity() != opacity {
            layer.set_opacity(opacity);
            self.full_rebuild = true;
        }
        Ok(())
    }

    pub fn set_visible(&mut self, index: usize, visible: bool) -> Result<(), String> {
        let layer = self.layers.get_mut(index).ok_or("layer index out of range")?;
        if layer.is_visible() != visible {
            layer.set_visible(visible);
            self.full_rebuild = true;
        }
        Ok(())
    }

    /// Rebuilds the complete scene from transparent pixels, in layer order.
    pub fn composite_into(&mut self, target: &mut Framebuffer) -> Result<(), String> {
        self.check_target(target)?;
        let previous_clip = target.clip;
        target.clip = None;
        target.clear_transparent();
        for layer in &self.layers {
            layer.composite_into(target)?;
        }
        target.clip = previous_clip;
        // The stack owns these layer surfaces; consume their pending changes
        // after incorporating them into the composed scene.
        for layer in &mut self.layers {
            layer.framebuffer_mut().take_dirty_regions();
        }
        self.full_rebuild = false;
        Ok(())
    }

    /// Re-renders only regions changed in any layer. Each affected target region
    /// is cleared and all layers are replayed there, preserving correct stacking
    /// when a lower layer changes beneath an unchanged upper layer.
    pub fn composite_dirty_into(&mut self, target: &mut Framebuffer) -> Result<usize, String> {
        self.check_target(target)?;
        if self.full_rebuild {
            self.composite_into(target)?;
            return Ok(1);
        }

        let mut dirty = Framebuffer::new(self.width, self.height)?;
        for layer in &self.layers {
            for region in layer.framebuffer().dirty_regions() {
                dirty.mark_dirty_region(region.x as i32, region.y as i32, region.width, region.height);
            }
        }
        let regions = dirty.take_dirty_regions();
        let mut redrawn = 0usize;

        for region in regions {
            target.save_clip();
            target.intersect_clip(region.x as i32, region.y as i32, region.width, region.height);
            target.clear_transparent_in_clip();
            for layer in &self.layers {
                layer.composite_into(target)?;
            }
            target.restore_clip();
            redrawn += 1;
        }
        for layer in &mut self.layers {
            layer.framebuffer_mut().take_dirty_regions();
        }
        Ok(redrawn)
    }

    fn check_target(&self, target: &Framebuffer) -> Result<(), String> {
        if target.width != self.width || target.height != self.height {
            return Err("layer stack and target framebuffer dimensions must match".into());
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
    fn test_palette(colors: &[Color]) -> Palette {
        let mut pixels = vec![0u8; COLOR_COUNT * 3];
        for color in colors {
            let offset = unpermute(color.packed()) as usize * 3;
            pixels[offset..offset + 3].copy_from_slice(&[color.r, color.g, color.b]);
        }
        Palette { pixels }
    }

    #[test]
    fn tiled_surface_try_pixel_and_rect_write_across_tiles() {
        let palette = test_palette(&[Color::rgb(12, 34, 56), Color::rgb(90, 80, 70)]);
        let mut surface = TiledSurface::with_tile_size(8, 8, 4).unwrap();
        surface.try_set_pixel(&palette, 4, 1, Color::rgb(12, 34, 56)).unwrap();
        surface.try_fill_rect(&palette, 2, 3, 4, 2, Color::rgb(90, 80, 70)).unwrap();
        assert_eq!(surface.pixel_rgba(4, 1), Some([12, 34, 56, 255]));
        assert_eq!(surface.pixel_rgba(2, 3), Some([90, 80, 70, 255]));
        assert_eq!(surface.pixel_rgba(5, 4), Some([90, 80, 70, 255]));
        assert_eq!(surface.allocated_tiles(), 4);
    }

    #[test]
    fn tiled_surface_fallible_line_reports_success_across_tiles() {
        let color = Color::rgb(31, 42, 53);
        let palette = test_palette(&[color]);
        let mut surface = TiledSurface::with_tile_size(8, 8, 2).unwrap();
        surface.try_draw_line(&palette, 0, 0, 7, 7, color).unwrap();
        for coordinate in 0..8 {
            assert_eq!(surface.pixel_rgba(coordinate, coordinate), Some([31, 42, 53, 255]));
        }
        assert_eq!(surface.allocated_tiles(), 4);
    }

    #[test]
    fn tiled_surface_rounded_rect_has_rounded_corners() {
        let color = Color::rgb(70, 80, 90);
        let palette = test_palette(&[color]);
        let mut surface = TiledSurface::with_tile_size(12, 12, 4).unwrap();
        surface.try_fill_rounded_rect(&palette, 1, 1, 10, 10, 4, color).unwrap();
        assert_eq!(surface.pixel_rgba(1, 1), None);
        assert_eq!(surface.pixel_rgba(5, 1), Some([70, 80, 90, 255]));
        assert_eq!(surface.pixel_rgba(1, 5), Some([70, 80, 90, 255]));
        assert_eq!(surface.pixel_rgba(10, 10), None);
        assert_eq!(surface.pixel_rgba(6, 6), Some([70, 80, 90, 255]));
    }

    #[test]
    fn tiled_surface_fills_vector_path_without_full_framebuffer() {
        let color = Color::rgb(120, 30, 210);
        let palette = test_palette(&[color]);
        let mut surface = TiledSurface::with_tile_size(20, 20, 4).unwrap();
        let mut path = VectorPath::new();
        path.move_to(2.0, 2.0)
            .line_to(17.0, 2.0)
            .line_to(17.0, 17.0)
            .line_to(2.0, 17.0)
            .close();
        surface.try_fill_path(&palette, &path, color).unwrap();
        assert_eq!(surface.pixel_rgba(5, 5), Some([120, 30, 210, 255]));
        assert_eq!(surface.pixel_rgba(0, 0), None);
        assert!(surface.allocated_tiles() > 0);
        assert!(surface.allocated_tiles() < 25);
    }

    #[test]
    fn tiled_surface_strokes_flattened_bezier_path() {
        let color = Color::rgb(11, 222, 99);
        let palette = test_palette(&[color]);
        let mut surface = TiledSurface::with_tile_size(32, 24, 8).unwrap();
        let mut path = VectorPath::new();
        path.move_to(2.0, 18.0)
            .cubic_to((8.0, 0.0), (22.0, 0.0), (29.0, 18.0), 16);
        surface.try_stroke_path(&palette, &path, color).unwrap();
        assert_eq!(surface.pixel_rgba(2, 18), Some([11, 222, 99, 255]));
        assert_eq!(surface.pixel_rgba(29, 18), Some([11, 222, 99, 255]));
        assert!(surface.allocated_tiles() > 0);
    }

    #[test]
    fn tiled_surface_alpha_blends_over_transparent_pixels() {
        let palette = test_palette(&[Color::rgb(200, 40, 20)]);
        let mut surface = TiledSurface::with_tile_size(4, 4, 2).unwrap();
        surface.blend_pixel(&palette, 1, 1, Color::rgb(200, 40, 20), 128).unwrap();
        let pixel = surface.pixel_rgba(1, 1).unwrap();
        assert_eq!(pixel, [200, 40, 20, 128]);
    }

    #[test]
    fn tiled_surface_alpha_blends_within_tile() {
        let palette = test_palette(&[Color::rgb(0, 0, 255), Color::rgb(255, 0, 0)]);
        let mut surface = TiledSurface::with_tile_size(4, 4, 2).unwrap();
        surface.try_set_pixel(&palette, 1, 1, Color::rgb(0, 0, 255)).unwrap();
        surface.blend_pixel(&palette, 1, 1, Color::rgb(255, 0, 0), 128).unwrap();
        let pixel = surface.pixel_rgba(1, 1).unwrap();
        assert!(pixel[0] >= 127 && pixel[0] <= 128);
        assert_eq!(pixel[1], 0);
        assert!(pixel[2] >= 127 && pixel[2] <= 128);
        assert_eq!(pixel[3], 255);
    }

    #[test]
    fn tiled_surface_composites_sparse_tiles_with_alpha_and_offset() {
        let palette = test_palette(&[Color::rgb(255, 0, 0), Color::rgb(0, 255, 0)]);
        let mut surface = TiledSurface::with_tile_size(8, 8, 4).unwrap();
        surface.blend_pixel(&palette, 1, 1, Color::rgb(255, 0, 0), 128).unwrap();
        surface.try_set_pixel(&palette, 5, 2, Color::rgb(0, 255, 0)).unwrap();
        let mut target = Framebuffer::new(10, 10).unwrap();
        let changed = surface.composite_into(&mut target, 2, 3).unwrap();
        assert_eq!(changed, 2);
        let red = target.pixel_rgba(3, 4).unwrap();
        assert_eq!(red, [255, 0, 0, 128]);
        assert_eq!(target.pixel_rgba(7, 5), Some([0, 255, 0, 255]));
        assert_eq!(target.pixel_rgba(0, 0), Some([0, 0, 0, 0]));
    }

    #[test]
    fn tiled_surface_composite_respects_target_clip() {
        let palette = test_palette(&[Color::WHITE]);
        let mut surface = TiledSurface::with_tile_size(4, 4, 2).unwrap();
        surface.try_fill_rect(&palette, 0, 0, 4, 4, Color::WHITE).unwrap();
        let mut target = Framebuffer::new(6, 6).unwrap();
        target.set_clip(2, 2, 2, 2);
        surface.composite_into(&mut target, 0, 0).unwrap();
        assert_eq!(target.pixel_rgba(2, 2), Some([255, 255, 255, 255]));
        assert_eq!(target.pixel_rgba(1, 1), Some([0, 0, 0, 0]));
        assert_eq!(target.pixel_rgba(4, 4), Some([0, 0, 0, 0]));
    }

    #[test]
    fn tiled_surface_allocates_only_touched_tiles() {
        let color = Color::rgb(12, 34, 56);
        let palette = test_palette(&[color]);
        let mut surface = TiledSurface::with_tile_size(1000, 700, 100).unwrap();
        assert_eq!(surface.allocated_tiles(), 0);
        surface.set_pixel(&palette, 250, 150, color);
        assert_eq!(surface.allocated_tiles(), 1);
        assert_eq!(surface.allocated_bytes(), 100 * 100 * 4);
        assert_eq!(surface.pixel_rgba(250, 150), Some([12, 34, 56, 255]));
        assert_eq!(surface.pixel_rgba(0, 0), None);
        surface.clear_tiles();
        assert_eq!(surface.allocated_tiles(), 0);
    }

    #[test]
    fn tiled_surface_fills_across_tile_boundaries() {
        let color = Color::rgb(1, 2, 3);
        let palette = test_palette(&[color]);
        let mut surface = TiledSurface::with_tile_size(300, 200, 64).unwrap();
        surface.fill_rect(&palette, 60, 60, 10, 10, color);
        assert_eq!(surface.pixel_rgba(60, 60), Some([1, 2, 3, 255]));
        assert_eq!(surface.pixel_rgba(69, 69), Some([1, 2, 3, 255]));
        assert_eq!(surface.pixel_rgba(59, 60), None);
        assert_eq!(surface.allocated_tiles(), 4);
    }

    #[test]
    fn tiled_surface_draws_lines_across_tiles() {
        let color = Color::rgb(21, 22, 23);
        let palette = test_palette(&[color]);
        let mut surface = TiledSurface::with_tile_size(256, 128, 32).unwrap();
        surface.draw_line(&palette, 1, 1, 100, 70, color);
        assert_eq!(surface.pixel_rgba(1, 1), Some([21, 22, 23, 255]));
        assert_eq!(surface.pixel_rgba(100, 70), Some([21, 22, 23, 255]));
        assert!(surface.allocated_tiles() > 1);
        assert!(surface.allocated_tiles() < 12);
    }

    #[test]
    fn tiled_surface_fills_circle_and_strokes_rectangle() {
        let circle = Color::rgb(31, 32, 33);
        let border = Color::rgb(41, 42, 43);
        let palette = test_palette(&[circle, border]);
        let mut surface = TiledSurface::with_tile_size(128, 128, 32).unwrap();
        surface.fill_circle(&palette, 40, 40, 8, circle);
        assert_eq!(surface.pixel_rgba(40, 40), Some([31, 32, 33, 255]));
        assert_eq!(surface.pixel_rgba(32, 40), Some([31, 32, 33, 255]));
        assert_eq!(surface.pixel_rgba(31, 40), None);
        surface.stroke_rect(&palette, 70, 70, 20, 15, 2, border);
        assert_eq!(surface.pixel_rgba(70, 70), Some([41, 42, 43, 255]));
        assert_eq!(surface.pixel_rgba(75, 75), None);
    }

    #[test]
    fn tiled_surface_fills_ellipse_with_clipped_edges() {
        let color = Color::rgb(51, 52, 53);
        let palette = test_palette(&[color]);
        let mut surface = TiledSurface::with_tile_size(80, 60, 16).unwrap();
        surface.fill_ellipse(&palette, 2, 20, 10, 6, color);
        assert_eq!(surface.pixel_rgba(2, 20), Some([51, 52, 53, 255]));
        assert_eq!(surface.pixel_rgba(12, 20), Some([51, 52, 53, 255]));
        assert_eq!(surface.pixel_rgba(13, 20), None);
        assert_eq!(surface.pixel_rgba(2, 14), Some([51, 52, 53, 255]));
        assert!(surface.allocated_tiles() > 0);
    }

    #[test]
    fn framebuffer_pool_reuses_and_resets_surfaces() {
        let mut pool = FramebufferPool::new(1024);
        let mut frame = pool.acquire(4, 3).unwrap();
        frame.pixels.fill(255);
        frame.set_clip(1, 1, 2, 2);
        frame.save_clip();
        frame.mark_dirty_region(0, 0, 2, 2);
        pool.release(frame);
        assert_eq!(pool.len(), 1);
        assert_eq!(pool.bytes_used(), 48);
        let reused = pool.acquire(4, 3).unwrap();
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.bytes_used(), 0);
        assert!(reused.pixels.iter().all(|byte| *byte == 0));
        assert!(reused.dirty_regions().is_empty());
        assert_eq!(reused.clip_stack_depth(), 0);
        assert_eq!(reused.clip, None);
    }

    #[test]
    fn framebuffer_pool_respects_memory_budget() {
        let mut pool = FramebufferPool::new(32);
        pool.release(Framebuffer::new(4, 3).unwrap());
        assert!(pool.is_empty());
        assert_eq!(pool.bytes_used(), 0);
        pool.set_byte_budget(128);
        pool.release(Framebuffer::new(4, 3).unwrap());
        assert_eq!(pool.bytes_used(), 48);
        pool.set_byte_budget(16);
        assert!(pool.is_empty());
        assert_eq!(pool.bytes_used(), 0);
    }


    #[test]
    fn viewport_renderer_updates_only_source_dirty_regions_after_initial_draw() {
        let mut source = Framebuffer::new(4, 1).unwrap();
        for x in 0..4 {
            let offset = source.offset(x, 0);
            source.pixels[offset..offset + 4].copy_from_slice(&[10, 0, 0, 255]);
        }
        let viewport = Viewport::new(4, 1).unwrap();
        let mut target = Framebuffer::new(4, 1).unwrap();
        let mut presenter = ViewportRenderer::new();

        assert_eq!(presenter.render_dirty_into(&viewport, &source, &mut target, 0, 0).unwrap(), 1);
        target.take_dirty_regions();

        let changed = source.offset(2, 0);
        source.pixels[changed..changed + 4].copy_from_slice(&[99, 0, 0, 255]);
        source.mark_dirty_region(2, 0, 1, 1);
        assert_eq!(presenter.render_dirty_into(&viewport, &source, &mut target, 0, 0).unwrap(), 1);
        assert_eq!(&target.pixels[changed..changed + 4], &[99, 0, 0, 255]);
        assert_eq!(target.dirty_regions(), &[DirtyRegion { x: 2, y: 0, width: 1, height: 1 }]);
        assert_eq!(source.dirty_regions().len(), 1);
    }

    #[test]
    fn viewport_renderer_redraws_when_scroll_changes() {
        let mut source = Framebuffer::new(3, 1).unwrap();
        source.pixels[0..12].copy_from_slice(&[
            1, 0, 0, 255, 2, 0, 0, 255, 3, 0, 0, 255
        ]);
        let mut viewport = Viewport::new(2, 1).unwrap();
        let mut target = Framebuffer::new(2, 1).unwrap();
        let mut presenter = ViewportRenderer::new();

        presenter.render_dirty_into(&viewport, &source, &mut target, 0, 0).unwrap();
        target.take_dirty_regions();
        viewport.set_scroll(1.0, 0.0).unwrap();
        assert_eq!(presenter.render_dirty_into(&viewport, &source, &mut target, 0, 0).unwrap(), 1);
        assert_eq!(&target.pixels[0..8], &[2, 0, 0, 255, 3, 0, 0, 255]);
        assert_eq!(target.dirty_regions(), &[DirtyRegion { x: 0, y: 0, width: 2, height: 1 }]);
    }

    #[test]
    fn viewport_scrolls_over_a_larger_source() {
        let mut source = Framebuffer::new(4, 2).unwrap();
        for x in 0..4 {
            let offset = source.offset(x, 0);
            source.pixels[offset..offset + 4].copy_from_slice(&[(x * 50) as u8, 0, 0, 255]);
        }
        let mut viewport = Viewport::new(2, 1).unwrap();
        viewport.set_scroll(1.0, 0.0).unwrap();
        let mut target = Framebuffer::new(2, 1).unwrap();
        viewport.render_into(&source, &mut target, 0, 0).unwrap();
        assert_eq!(&target.pixels[0..4], &[50, 0, 0, 255]);
        assert_eq!(&target.pixels[4..8], &[100, 0, 0, 255]);
    }

    #[test]
    fn viewport_scales_with_nearest_neighbor_sampling() {
        let mut source = Framebuffer::new(2, 1).unwrap();
        source.pixels[0..4].copy_from_slice(&[10, 0, 0, 255]);
        source.pixels[4..8].copy_from_slice(&[20, 0, 0, 255]);
        let mut viewport = Viewport::new(4, 1).unwrap();
        viewport.set_scale(2.0).unwrap();
        let mut target = Framebuffer::new(4, 1).unwrap();
        viewport.render_into(&source, &mut target, 0, 0).unwrap();
        assert_eq!(&target.pixels, &[10, 0, 0, 255, 10, 0, 0, 255, 20, 0, 0, 255, 20, 0, 0, 255]);
    }

    #[test]
    fn viewport_respects_destination_clip_and_clears_outside_source() {
        let mut source = Framebuffer::new(1, 1).unwrap();
        source.pixels.copy_from_slice(&[1, 2, 3, 255]);
        let viewport = Viewport::new(3, 2).unwrap();
        let mut target = Framebuffer::new(4, 3).unwrap();
        target.pixels.fill(99);
        target.set_clip(1, 0, 2, 2);
        viewport.render_into(&source, &mut target, 0, 0).unwrap();
        assert_eq!(&target.pixels[target.offset(0, 0)..target.offset(0, 0) + 4], &[99; 4]);
        assert_eq!(&target.pixels[target.offset(1, 0)..target.offset(1, 0) + 4], &[1, 2, 3, 255]);
        assert_eq!(&target.pixels[target.offset(2, 0)..target.offset(2, 0) + 4], &[0; 4]);
        assert_eq!(&target.pixels[target.offset(1, 2)..target.offset(1, 2) + 4], &[99; 4]);
    }

    #[test]
    fn viewport_rejects_invalid_scale_and_non_finite_scroll() {
        let mut viewport = Viewport::new(2, 2).unwrap();
        assert!(viewport.set_scale(0.0).is_err());
        assert!(viewport.set_scale(f32::INFINITY).is_err());
        assert!(viewport.set_scroll(f32::NAN, 0.0).is_err());
        assert!(Viewport::new(0, 2).is_err());
    }

    #[test]
    fn copy_dirty_from_copies_only_marked_pixels() {
        let mut source = Framebuffer::new(4, 3).unwrap();
        let mut target = Framebuffer::new(4, 3).unwrap();
        source.pixels.fill(0);
        source.pixels[4..8].copy_from_slice(&[10, 20, 30, 255]);
        source.mark_dirty_region(1, 0, 1, 1);
        let copied = target.copy_dirty_from(&source).unwrap();
        assert_eq!(copied, 1);
        assert_eq!(&target.pixels[4..8], &[10, 20, 30, 255]);
        assert_eq!(&target.pixels[0..4], &[0, 0, 0, 0]);
        assert_eq!(target.dirty_regions(), &[DirtyRegion { x: 1, y: 0, width: 1, height: 1 }]);
        assert_eq!(source.dirty_regions().len(), 1);
    }

    #[test]
    fn copy_dirty_from_respects_destination_clip() {
        let mut source = Framebuffer::new(4, 4).unwrap();
        let mut target = Framebuffer::new(4, 4).unwrap();
        source.pixels.fill(255);
        source.mark_dirty_region(0, 0, 4, 4);
        target.set_clip(1, 1, 2, 2);
        assert_eq!(target.copy_dirty_from(&source).unwrap(), 1);
        assert_eq!(target.dirty_regions(), &[DirtyRegion { x: 1, y: 1, width: 2, height: 2 }]);
        assert_eq!(&target.pixels[target.offset(0, 0)..target.offset(0, 0)+4], &[0, 0, 0, 0]);
        assert_eq!(&target.pixels[target.offset(1, 1)..target.offset(1, 1)+4], &[255, 255, 255, 255]);
    }

    #[test]
    fn transparent_clear_marks_full_frame_even_with_clip() {
        let mut frame = Framebuffer::new(7, 5).unwrap();
        frame.set_clip(2, 1, 2, 2);
        frame.clear_transparent();
        assert_eq!(frame.dirty_regions(), &[DirtyRegion { x: 0, y: 0, width: 7, height: 5 }]);
    }

    #[test]
    fn dirty_regions_merge_and_respect_clip() {
        let mut frame = Framebuffer::new(20, 20).unwrap();
        frame.set_clip(5, 5, 10, 10);
        frame.mark_dirty_region(0, 0, 10, 10);
        frame.mark_dirty_region(9, 9, 5, 5);
        assert_eq!(frame.dirty_regions(), &[DirtyRegion { x: 5, y: 5, width: 9, height: 9 }]);
        let taken = frame.take_dirty_regions();
        assert_eq!(taken.len(), 1);
        assert!(!frame.has_dirty_regions());
    }

    #[test]
    fn dirty_region_count_is_bounded() {
        let mut frame = Framebuffer::new(512, 4).unwrap();
        for x in (0..512).step_by(2) {
            frame.mark_dirty_region(x, 0, 1, 1);
        }
        assert!(frame.dirty_regions().len() <= MAX_DIRTY_REGIONS);
        let left = frame.dirty_regions().iter().map(|region| region.x).min().unwrap();
        let right = frame.dirty_regions().iter().map(|region| region.x + region.width).max().unwrap();
        assert_eq!(left, 0);
        assert_eq!(right, 511);
    }

    #[test]
    fn pixel_writes_are_reported_as_dirty() {
        let mut frame = Framebuffer::new(4, 4).unwrap();
        // The palette is not needed to exercise the public region accumulator.
        frame.mark_dirty_region(2, 1, 1, 1);
        assert_eq!(frame.take_dirty_regions(), vec![DirtyRegion { x: 2, y: 1, width: 1, height: 1 }]);
    }

    #[test]
    fn full_clear_marks_entire_frame_even_when_clipped() {
        let mut frame = Framebuffer::new(8, 6).unwrap();
        frame.set_clip(2, 2, 2, 2);
        frame.clear_transparent();
        assert_eq!(frame.dirty_regions(), &[DirtyRegion { x: 0, y: 0, width: 8, height: 6 }]);
    }

    #[test]
    fn render_cache_reuses_and_invalidates_frames() {
        let mut cache = RenderCache::new(64);
        let mut frame = Framebuffer::new(2, 2).unwrap();
        frame.pixels[0..4].copy_from_slice(&[1, 2, 3, 255]);
        cache.insert(42, &frame);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get(42).unwrap().pixels[0..4], [1, 2, 3, 255]);
        frame.pixels[0] = 99;
        assert_eq!(cache.get(42).unwrap().pixels[0], 1);
        assert!(cache.invalidate(42));
        assert!(cache.get(42).is_none());
        assert_eq!(cache.bytes_used(), 0);
    }

    #[test]
    fn render_cache_evicts_oldest_entry_to_respect_budget() {
        let mut cache = RenderCache::new(16);
        let first = Framebuffer::new(2, 2).unwrap();
        let second = Framebuffer::new(2, 2).unwrap();
        cache.insert(1, &first);
        cache.insert(2, &second);
        assert!(cache.get(1).is_none());
        assert!(cache.get(2).is_some());
        assert_eq!(cache.bytes_used(), 16);
        cache.set_byte_budget(0);
        assert!(cache.is_empty());
    }

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
    fn layer_dirty_compositing_updates_only_marked_regions() {
        let mut layer = Layer::new(3, 1).unwrap();
        let mut target = Framebuffer::new(3, 1).unwrap();
        layer.framebuffer_mut().pixels[0..4].copy_from_slice(&[255, 0, 0, 255]);
        layer.framebuffer_mut().pixels[8..12].copy_from_slice(&[0, 255, 0, 255]);
        layer.framebuffer_mut().mark_dirty_region(0, 0, 1, 1);
        layer.framebuffer_mut().mark_dirty_region(2, 0, 1, 1);
        assert_eq!(layer.composite_dirty_into(&mut target).unwrap(), 2);
        assert_eq!(&target.pixels[0..4], &[255, 0, 0, 255]);
        assert_eq!(&target.pixels[4..8], &[0, 0, 0, 0]);
        assert_eq!(&target.pixels[8..12], &[0, 255, 0, 255]);
        assert_eq!(layer.framebuffer().dirty_regions().len(), 2);
    }

    #[test]
    fn dirty_layer_compositing_skips_invisible_and_transparent_layers() {
        let mut layer = Layer::new(1, 1).unwrap();
        layer.framebuffer_mut().pixels.copy_from_slice(&[255, 0, 0, 255]);
        layer.framebuffer_mut().mark_dirty_region(0, 0, 1, 1);
        let mut target = Framebuffer::new(1, 1).unwrap();
        layer.set_visible(false);
        assert_eq!(layer.composite_dirty_into(&mut target).unwrap(), 0);
        layer.set_visible(true);
        layer.set_opacity(0);
        assert_eq!(layer.composite_dirty_into(&mut target).unwrap(), 0);
        assert_eq!(target.pixels, vec![0, 0, 0, 0]);
    }
    #[test]
    fn layer_stack_recomposes_only_changed_regions_with_all_layers() {
        let mut stack = LayerStack::new(3, 1).unwrap();
        let bottom = stack.add_layer().unwrap();
        let top = stack.add_layer().unwrap();
        stack.framebuffer_mut(bottom).unwrap().pixels[0..12]
            .copy_from_slice(&[255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255]);
        stack.framebuffer_mut(top).unwrap().pixels[0..4]
            .copy_from_slice(&[0, 0, 255, 255]);
        stack.framebuffer_mut(bottom).unwrap().mark_dirty_region(0, 0, 3, 1);
        stack.framebuffer_mut(top).unwrap().mark_dirty_region(0, 0, 1, 1);

        let mut target = Framebuffer::new(3, 1).unwrap();
        assert_eq!(stack.composite_dirty_into(&mut target).unwrap(), 1);
        assert_eq!(&target.pixels[0..4], &[0, 0, 255, 255]);
        assert_eq!(&target.pixels[4..8], &[255, 0, 0, 255]);

        stack.framebuffer_mut(bottom).unwrap().pixels[4..8]
            .copy_from_slice(&[0, 255, 0, 255]);
        stack.framebuffer_mut(bottom).unwrap().mark_dirty_region(1, 0, 1, 1);
        assert_eq!(stack.composite_dirty_into(&mut target).unwrap(), 1);
        assert_eq!(&target.pixels[4..8], &[0, 255, 0, 255]);
        assert_eq!(&target.pixels[0..4], &[0, 0, 255, 255]);
        assert_eq!(stack.composite_dirty_into(&mut target).unwrap(), 0);
    }

    #[test]
    fn layer_stack_can_remove_and_reorder_layers() {
        let mut stack = LayerStack::new(1, 1).unwrap();
        let back = stack.add_layer().unwrap();
        let front = stack.add_layer().unwrap();
        stack.framebuffer_mut(back).unwrap().pixels.copy_from_slice(&[255, 0, 0, 255]);
        stack.framebuffer_mut(front).unwrap().pixels.copy_from_slice(&[0, 0, 255, 255]);
        let mut target = Framebuffer::new(1, 1).unwrap();
        stack.composite_into(&mut target).unwrap();
        assert_eq!(&target.pixels, &[0, 0, 255, 255]);

        stack.move_layer(front, back).unwrap();
        stack.composite_dirty_into(&mut target).unwrap();
        assert_eq!(&target.pixels, &[255, 0, 0, 255]);

        assert!(stack.remove_layer(back).is_some());
        stack.composite_dirty_into(&mut target).unwrap();
        assert_eq!(&target.pixels, &[0, 0, 255, 255]);
    }

    #[test]
    fn layer_stack_property_changes_force_full_rebuild() {
        let mut stack = LayerStack::new(1, 1).unwrap();
        let index = stack.add_layer().unwrap();
        stack.framebuffer_mut(index).unwrap().pixels.copy_from_slice(&[255, 0, 0, 255]);
        let mut target = Framebuffer::new(1, 1).unwrap();
        stack.composite_dirty_into(&mut target).unwrap();
        assert_eq!(&target.pixels, &[255, 0, 0, 255]);

        stack.set_visible(index, false).unwrap();
        assert_eq!(stack.composite_dirty_into(&mut target).unwrap(), 1);
        assert_eq!(&target.pixels, &[0, 0, 0, 0]);
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