---
title: Test fonts and synthetic sfnt builders (rikka-test-fonts)
description: Shared test-only helpers: session font lookup via $FONTCONFIG_FILE, a big-endian Writer, sfnt assembly.
about: crates/text/rikka-test-fonts
tags: [text, testing]
---

- Dependency-free `testonly` crate that the `crates/text` crates and rikka-symbols-scanner take only in their `rikka_test` targets, so libraries never link it. `benchmarks/text` (text-bench) is the one regular dependent.
- `font_files()` walks the `<dir>` entries of `$FONTCONFIG_FILE` once (`~/` resolves against `$HOME`, skipped without it; prefixed dirs are ignored). A missing `$FONTCONFIG_FILE` panics.
- `find_font(name)` gives `Option<&Path>`; `font_path(name)` and `font_bytes(name)` panic when the font is not installed. `loaded_font(name)` reads a font once per process and leaks it as `&'static [u8]`, which `rikka_text_shape::ShapeFace` needs; `find_loaded_font(name)` is the same without the panic. Both accept `Font.ttf~HVAR` names, which load the font with that table removed.
- `Writer` appends big-endian fields (`u8`…`i32`, `u24`, `fixed`, `f2dot14`, `raw`, `align`) and patches offsets with `set_u8`/`set_u16`/`set_u24`/`set_u32`; `finish()` takes the bytes.
- `sfnt(&[(tag, data)])` assembles a sorted, 4-byte aligned table directory; `head(units_per_em)` and `maxp(glyph_count)` are the minimal required tables. `without_table(font, tag)` rebuilds a font without one table.
