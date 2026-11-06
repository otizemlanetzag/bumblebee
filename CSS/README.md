# Bumblebee pixel renderer

This directory contains the first independent rendering subsystem for Bumblebee. It is a CPU-side pixel renderer; it does not implement HTML parsing or CSS parsing yet.

## RGB palette

The renderer uses a 4096 × 4096 RGB PNG as a complete color source. Every 24-bit RGB value appears exactly once. The palette order is a deterministic reversible permutation, so a requested color can be mapped to its unique source pixel without building a large hash map.

Generate the canonical palette:

```sh
cargo run --manifest-path CSS/Cargo.toml -- generate-palette CSS/all-rgb-colors.png
```

The image is generated from the exact 24-bit RGB domain. Do not use an arbitrary gradient or noisy image as the canonical palette: it must pass the loader's full validation.

At startup, load and validate it once:

```rust
use bumblebee_renderer::{Color, Palette, Framebuffer};

let palette = Palette::load("CSS/all-rgb-colors.png")?;
let mut frame = Framebuffer::new(800, 600)?;
frame.clear(&palette, Color::WHITE);
# Ok::<(), String>(())
```

## Current drawing operations

- Individual pixels and alpha blending
- Filled rectangles and rectangle borders
- Lines
- Filled circles
- Horizontal two-color linear gradients
- PNG framebuffer export for testing

Run a visual smoke-test image:

```sh
cargo run --manifest-path CSS/Cargo.toml -- demo CSS/all-rgb-colors.png output.png
```

## Important limitations

This is a first rendering core, not a complete browser renderer. Rounded rectangles, ellipses, polygon/path rasterization, SVG parsing, CSS gradients beyond the horizontal two-stop case, transforms, clipping stacks, compositing layers, text integration, video/image surfaces, OS window presentation, and GPU acceleration are not implemented yet.

The PNG palette is a source of exact RGB values, not a way to represent alpha. Alpha is handled separately by the framebuffer. For speed, palette inversion is cached and drawing operations sample a color once where practical.
