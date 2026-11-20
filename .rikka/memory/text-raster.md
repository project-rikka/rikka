---
title: Glyph rasterizer (rikka-text-raster)
description: Rasterizer for masks, color glyphs (COLR, CBDT, sbix, SVG bytes) and the GPU curve blob for large glyphs.
about: crates/text/rikka-text-raster
tags: [text, raster]
---

- Sits on `rikka-text-font` (plus rikka-simd, rikka-types, rikka-deflate, rikka-png and rikka-image-resample). `Rasterizer` is the single entry point and owns all scratch, so steady-state rendering allocates nothing.
- `render(font, &GlyphRequest, &mut GlyphImage)` picks COLRv1 → SVG → COLRv0 → CBDT → sbix → outline mask and returns `Result<Rendered>`; every entry point fails with the crate's `Error`. SVG comes back as decompressed document bytes for `rikka-svg`.
- `GlyphRequest::new(glyph, ppem)` builds with location, `subpixel_x`, `subpixel_y` (a y-up rise, honoured by outline, COLR and bitmap paths), `.synthesis(rikka_text_font::Synthesis)`, foreground color, palette and a color switch.
- `Synthesis { bold, oblique }` lives in rikka-text-font, shared with rikka-text-catalog and rikka-text-layout; bold adds `extra_advance_em()` that layout adds. `distortion.rs` turns it into embolden and skew amounts.
- `PreparedOutline` is a reusable outline for one (face, glyph, location): `prepare_outline` loads it once, `render_prepared` rasters it at any size, subpixel offset and synthesis. Callers cache it; `render_outline` is the same path through internal scratch, so both give identical masks. `trace(synthesis, ppem, &device, &mut impl OutlineSink)` emits it as quads/cubics through `device` (from y-up px at the pen), emboldened and skewed exactly like the mask, closing every contour. `OutlineCache<K>` is the byte-budget CLOCK cache of prepared outlines (8 MiB by default, `with_budget`).
- A prepared outline keeps up to four flattenings in font units, one per half-octave size class. Emboldening miters are computed lazily from edge normals the first time a synthetic-bold size needs them and are interpolated along the flattening. `heap_bytes()` reports its memory for byte-budget caches.
- `GlyphImage` uses baseline-origin placement (`left` from the pen, `top` above the baseline). Masks are linear A8 coverage. Color is premultiplied sRGB RGBA8.
- Paths, flattening and the mask pipeline are `rikka-vector`'s (`Path`, `flatten`, `Rasterizer::mask_in`); `mask.rs` only places a prepared outline's points into its `Polylines`. Coverage is the absolute value of the exact signed area of the flattened polygon. COLR paints through `rikka_vector::Canvas`, clipping to glyph outlines with `Contours::Simple`. Outlines over 2^20 points, flattenings over 2^18 points and glyphs over 2^20 spans fail cleanly.
- SIMD goes through rikka-simd: the mask pipeline is generic over `Simd`/`Float` lanes and entered with `rikka_simd::dispatch!`, so lane math is written once and every `rikka_simd::Level` is bit-identical.
- CBDT/sbix PNG strikes decode through `rikka-png` (`Decoder::rgba8`, 16384 px and 64 MiB at most) and gzip SVG documents through `rikka-deflate`; decoded strikes stay in a 16-entry LRU and are linearized while resampling in linear premultiplied f32 through `rikka-image-resample` (`Footprint::Coverage`). COLR gradients interpolate in linear-light premultiplied colour (OpenType CPAL rule) and layers composite in premultiplied sRGB.
- `encode_curves` writes the GPU curve blob. Its layout is defined by the `CurveBlob` constants, and `coverage_at` is the CPU reference the shader ports.
- Speed against swash, FreeType and fontdue: `bazel run --config=bench //benchmarks/text:raster` (`RikkaPrepared` uses prepared outlines).
