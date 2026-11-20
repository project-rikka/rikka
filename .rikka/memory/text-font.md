---
title: OpenType parser (rikka-text-font)
description: Zero-copy, never-panicking OpenType reader: tables, cmap, variations, outlines, metrics, layout and color readers.
about: crates/text/rikka-text-font
tags: [text, fonts]
---

- Bottom of the text stack beside `rikka-text-unicode`; depends only on `thiserror`. `rikka-text-shape`, `rikka-text-raster` and `rikka-text-catalog` read fonts through it.
- `FontFile::new(bytes)` opens sfnt, TTC and OTC; `Font` is a `Copy` handle whose known table offsets are resolved once, so table access is O(1).
- Every reader is a `Copy` view over the bytes; malformed data yields `None`, never a panic. Composites, subrs and COLR paint graphs carry recursion and cycle limits.
- `font.charmap()` picks the best cmap subtable; `map_variant` handles format 14; `for_each_mapping` feeds coverage indexing.
- Variations: `font.location([(tag, value)])` normalizes and applies avar v1/v2; the default instance is the empty `Location`.
- `font.outline(glyph, &location, &mut OutlineScratch, &mut sink)` emits font-unit f32 paths (quads for glyf, cubics for CFF/CFF2, never rounded) and returns only success; `font.glyph_bounds` gives bounds.
- CFF/CFF2 header, INDEX positions, top, private and FD dicts are parsed once per table into the `OutlineScratch` (keyed by table address, length and prefix), so reuse one scratch per thread.
- `font.metrics(&location)` and `font.glyph_metrics(&location)` apply MVAR and HVAR/VVAR (gvar phantom points otherwise).
- Reference-exact 16.16 paths for rikka-text-shape: `ItemVariationStore::fixed_scalars` + `delta_fixed`, `MetricsVariations::advance_delta_fixed`, `Gdef::var_store`, and `font.phantom_advances(coords)` (gvar phantom points through `USE_MY_METRICS`).
- Layout readers (`gsub`, `gpos`, `gdef`, `kern`) expose every subtable format with extensions unwrapped; the shaping engine lives in `rikka-text-shape`. `Lookup::subtable_location` gives a lifetime-free `SubtableLocation` and `LookupSubtable::from_location(table, location)` rebuilds the view; subtable views parse Coverage and ClassDef only when asked.
- Color: `cpal`, `color_glyph(g).paint(coords, &mut impl ColorPainter)` for COLR v0/v1, `bitmap_glyph(g, ppem)` for CBDT/EBDT, `sbix`, `svg_document`.
- `Synthesis { bold, oblique }` (synthetic emboldening and slant) is the one synthesis vocabulary for catalog, layout and raster.
- Tests find session fonts and build synthetic fonts with `rikka-test-fonts`.
