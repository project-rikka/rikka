---
title: PSD compositor (crates/graphics/rikka-psd-render)
description: rikka-psd-render composites a rikka_psd::Document on the CPU: blend modes, masks, groups, knockout, adjustments.
about: crates/graphics/rikka-psd-render
tags: [psd, rendering]
---

- Public surface: `render_rgba` and `refresh_composite` (what Paint and Viewer use), plus `Renderer::new(&doc).render()` returning a premultiplied `Raster` (`pixel`, `width`, `height`) in the document's own channels, for checks in ink space. Everything else (`Mask`, the crate-private `Space`, vector rasterizing, region renders) is internal and tested through those.
- Channel spaces (`space.rs`): gray, RGB, inverted-ink CMYK, encoded Lab, built from the document's depth; Lab channel coding and BT.601 gray come from rikka-psd's `lab_encoded`/`lab_decoded`/`rgb_to_gray`.
- Layer opacity and global masks are applied by lerping the canvas toward a styled copy (`Raster::lerp_from(target, amount, masks)`, per-row coverage factors), so effects, knockout and blend modes compose like non-isolated layers. A layer without effects, clips, knockout, blend-if or channel restrictions skips the copy: `composite_faded` blends and lerps straight into each canvas row with identical arithmetic; effect layers at full opacity without global masks draw directly onto the canvas.
- Hot pixel work runs as rikka-simd kernels (`lanes.rs`: `Kernel`/`sweep` with a single-lane tail, `Pixels<V, C>` transposed RGB loads, `by_channels!`), dispatched per row or call. Compositing and LUT toning load interleaved destination pixels with `Pixels`, work on every channel in registers and store once; the layer's straightened source row and decoded bands are `planar::Planar` scratch.
- Separable blend modes are `Blend` types (`blend/separable.rs`) chosen once per row by `blending()`; Dissolve, Darker/Lighter Color and Hue/Saturation/Color/Luminosity run per pixel. Adjustment layers stream row by row without cloning the canvas; their blend modes run per pixel (`blend/adjusted.rs`).
- Layers decode in 64-row bands straight into the interleaved raster.
- Pass-through groups draw onto the live canvas; at full opacity with no mask or clips they draw straight into it (snapshotting the canvas only when a child knocks out shallowly); fill < 100% or clipped layers isolate their adjustments to the group's shape; knockout or effects make them isolated.
- Deep knockout reaches the Background layer (a bottom layer without a transparency channel) only through pass-through groups; isolated groups stop it at their own backdrop.
- When a vector mask exists and the user mask has flag 0x08, the `-2` channel is the rasterized vector mask and the real pixel mask is `-3` (`LayerMask.real`).
- Stored layer pixels are used for text, smart objects and shapes; fill layers without pixels are synthesized from `SoCo`/`GdFl`/`PtFl` + vector mask + `vstk`.
- Vector masks and `vstk` strokes go through `rikka-vector` (`Rasterizer` with `Contours::Overlapping`, `Stroker`); subpath `rule == 2` is non-zero winding, otherwise even-odd; operation `Unspecified` merges into the previous component.
- Layer-style sizes are used as stored; the descriptor's `Scl ` is a resolution note and is ignored.
