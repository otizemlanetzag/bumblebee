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
- Dirty-region tracking for pixel writes, alpha blends, all filled shape rasterizers, layer compositing, and full-frame clears; `take_dirty_regions()` returns and resets the pending rectangles
- Incremental framebuffer synchronization with `copy_dirty_from()`, which copies only changed rectangles, respects the destination clip, and leaves source dirty state intact for multiple consumers
- Independent off-screen layers with visibility toggles and opacity
- Scrollable and scalable `Viewport` windows over larger framebuffers, with source-pixel scrolling, nearest-neighbor scaling, destination clipping, transparent out-of-source areas, and no consumption of source dirty regions; `ViewportRenderer` tracks view changes and redraws only mapped dirty source regions between full redraws\n- Ordered `LayerStack` scene composition with layer addition, removal, z-order changes, opacity and visibility controls, plus incremental dirty-region recomposition: when a layer changes, only affected target rectangles are cleared and all layers are replayed inside those rectangles, preserving correct overlap; visibility and opacity changes trigger a full rebuild
- Bounded render cache for reusable framebuffer snapshots, with explicit invalidation and oldest-first memory-budget eviction
- Bounded `FramebufferPool` for reusing temporary off-screen surfaces by exact dimensions; acquired surfaces are reset to transparent with clean clip and dirty state, and retained allocations are capped by a byte budget
- Sparse `TiledSurface` for large logical drawing areas: pixel memory is allocated only for touched tiles (128×128 by default), with allocated-tile and byte accounting; includes fallible `try_set_pixel()` and `try_fill_rect()` APIs, convenience write methods, per-pixel alpha blending, Bresenham lines, filled circles and ellipses, stroked rectangles, and source-over compositing into clipped framebuffers
- Incremental layer compositing with `composite_dirty_into()`, which scans only pending source dirty regions and preserves them for multiple targets; perform a full scene recomposition after changing layer visibility or opacity
- Source-over alpha compositing of transparent layers into a target framebuffer
- Transparent framebuffer clearing for layer construction
- Filled rectangles and rectangle borders
- Lines
- Filled circles and ellipses
- Rounded rectangles and rounded borders
- Filled polygons using an even-odd scanline fill rule
- Open and closed polylines
- Quadratic and cubic Bézier curves
- Reusable vector paths with move, line, curve, and close commands
- Affine path transforms: translation, scaling, rotation, and matrix composition
- Framebuffer clipping rectangles, including nested intersection, save/restore stack, and reset; filled ellipses, rounded rectangles, polygons, and vector-path fills respect the active clip
- Anti-aliased line drawing with coverage blending
- Even-odd filling across multiple path contours (including holes)
- Horizontal two-color linear gradients
- Elliptical radial gradients
- PNG framebuffer export for testing

Run a visual smoke-test image:

```sh
cargo run --manifest-path CSS/Cargo.toml -- demo CSS/all-rgb-colors.png output.png
```

## Important limitations

This is a first rendering core, not a complete browser renderer. SVG parsing and SVG path-data tokenization are not implemented yet; vector paths must currently be constructed through the Rust API. Full CSS gradient syntax and color-stop handling, anti-aliasing for all shape types, text integration, video/image surfaces, OS window presentation, and GPU acceleration are also pending. Affine transforms currently apply to vector paths, not to every primitive.

The PNG palette is a source of exact RGB values, not a way to represent alpha. Alpha is handled separately by the framebuffer. For speed, palette inversion is cached and drawing operations sample a color once where practical.

`LayerStack` owns its layer surfaces and consumes their pending dirty regions after composing them. Draw through `LayerStack::framebuffer_mut()` so changes are tracked; use its visibility/opacity setters so configuration changes trigger a safe full-scene rebuild.

Dirty regions are clipped to the framebuffer and active clip, and overlapping or adjacent regions are merged. The list is capped at 64 regions; if it grows beyond that, it is conservatively collapsed into one bounding rectangle to keep tracking memory bounded. Full-frame clear operations deliberately mark the entire framebuffer dirty, regardless of the active drawing clip, because they clear every pixel.
\n`Viewport` is a CPU-side view onto an existing framebuffer. Its scroll offsets are measured in source pixels, and its scale is destination pixels per source pixel. `render_into()` samples with nearest-neighbor filtering, clears visible areas that fall outside the source to transparency, respects the destination clip, and leaves the source's dirty-region state untouched. Changing the viewport settings does not redraw anything by itself. Use `ViewportRenderer::render_dirty_into()` for incremental presentation: its first call and any viewport/origin/surface-size change redraw the full view; later calls map source dirty rectangles into destination rectangles. Call `invalidate()` to force a full redraw. The renderer does not consume source dirty regions, so other consumers can still use them.\n

`FramebufferPool` reduces repeated allocation and initialization churn for temporary render targets. It only retains released surfaces within its configured memory budget; it does not reduce the memory footprint of framebuffers that remain active. Use `acquire(width, height)` and return temporary surfaces with `release(framebuffer)`.


`TiledSurface` is intended for very large or mostly empty drawing areas. It allocates a tile only when a pixel or filled rectangle touches that tile; untouched areas consume no pixel-buffer memory. `pixel_rgba()` returns `None` for an unallocated tile (transparent by default), and `clear_tiles()` releases all tile allocations. The tiled API supports pixel writes, filled and stroked rectangles, Bresenham lines, filled circles and ellipses. Other drawing primitives and direct presentation are not yet integrated.
