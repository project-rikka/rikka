---
title: CPU vector graphics library (rikka-vector)
description: rikka-vector's Path, flattening, two coverage rasterizers, Stroker, paints and the layered Canvas under text and SVG.
about: crates/graphics/rikka-vector
tags: [vector, raster]
---

- Depends only on rikka-simd, rikka-types and thiserror. `rikka-text-raster` (glyph masks, COLR), `rikka-svg` and `rikka-psd-render` (vector masks, strokes) all draw through it; none keeps a private rasterizer, stroker or path type.
- `Path` holds `Verb`s and `rikka_types::Point`s (2^20 point limit): `move_to/line_to/quad_to/cubic_to/close` (SVG arcs become cubics in `rikka-path-data`), `add_rect/add_rounded_rect/add_ellipse`, `transformed`, exact `bounds()`. `Transform` is the row-vector affine `{xx,yx,xy,yy,dx,dy}`; `a.then(b)` applies `a` first.
- `Rasterizer::flatten(path, transform, tolerance)` flattens into `Polylines`; callers pass `GLYPH_FLATNESS` (0.074 px, glyph masks and COLR, whose pinned output depends on it) or `FILL_FLATNESS` (0.02 px, SVG and PSD; coarser visibly shrinks small circles). It has two coverage paths, chosen by `Contours`:
  - `Simple`: SIMD signed-area accumulation, exact for contours that neither overlap nor touch with opposite direction. Glyph masks (`mask`, `mask_in`) always use it, so font output is unchanged by anything else.
  - `Overlapping`: `fill/sweep.rs` evaluates the winding on 32 sample lines per pixel row with exact horizontal spans. Use it for arbitrary content: stroke outlines, SVG and PSD paths.
- `Stroker::outline(path, &Stroke, tolerance, &mut Path)` turns a stroke (joins, caps, miter limit, dashes) into a fill outline; rasterize it as `Overlapping`. A polyline that would need more than 2^20 dashes is stroked solid.
- `Paint` is `Solid`, `Gradient` (linear, radial with focal circle, sweep; `Spread`; `Interpolation::Srgb` or `LinearLight`) or `Pattern` (a `Pixmap`, `Tiling`, `Sampling`). Degenerate gradients paint nothing; callers substitute a solid.
- `Canvas` composites premultiplied sRGB RGBA8; `set_antialiasing(Antialiasing::Disabled)` makes fills, strokes and clips aliased (one centre sample per pixel, `Rasterizer::aliased_coverage`) until set back; it flattens fills, clips and stroke outlines at `FILL_FLATNESS` unless built `.with_flatness(GLYPH_FLATNESS)` (COLR): `begin(w,h)`, `fill` and `stroke` (both `Overlapping`), `paint` (fills the clip), `push_clip(path, transform, rule, contours)`/`push_clip_stencil`/`push_clip_area`, `pop_clip`, `push_layer`/`push_layer_within(area)` (inside the clip)/`push_layer_covering(area)` (exactly `area`, even outside the canvas), `pop_layer(&Composite { mode, opacity, mask })`, `layer_mut()` for in-place filters.
- A clip applies its coverage to every draw separately. To clip several overlapping draws as one, push the clip, push a layer, draw, pop both; otherwise shared antialiased edges bleed.
- `Pixmap` is premultiplied RGBA8 (`from_straight`, `into_straight`); `Mask` is A8 (`from_alpha`, `from_luminance`, `from_linear_luminance`). `BlendMode::composite(source, backdrop)` implements the 28 Porter-Duff and blend modes.
