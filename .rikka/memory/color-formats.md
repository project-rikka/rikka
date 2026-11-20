---
title: Colour formats (rikka-color-formats)
description: Configurable colour format templates, colour spaces, names, shades and export, shared by shell and Settings.
about: crates/foundation/rikka-color-formats
tags: [color, foundation]
---

- `Rgb` is an opaque 8-bit sRGB sample (`parse_hex`, `hex`, `of(Color)`, serde as `rrggbb`); spaces hang off it (`hsv/hsl/hsi/hwb/cmyk/xyz/lab/oklab/oklch/natural`), plus `name()` and `variations()`.
- Maths follows the reference colour-format helper on purpose: .NET `GetHue` in f32, banker's rounding (`round_ties_even`), its XYZ/Lab/Oklab constants; tests check its reference tables. Negative zero is written `0`.
- `Template::parse` reads the reference format language (`%Re` … `%Na`, formatter letters `b h H x X f F`, `i` for Lab); an empty template renders hex. `Parameter`/`Formatter` list codes and localized labels for help screens.
- The format list rows are the settings catalog's `rikka_settings::session::color_picker::Format { name, format, shown }` with `built_in_formats()` (the reference's 16 defaults, HEX, RGB, HSL shown) and `HEX`; this crate depends on rikka-settings for them. `template_named(formats, name)` falls back to hex.
- `ColorName` is the reference's HSL bucket naming; labels come from the crate's own catalog (`CATALOGS`, embed it in hosts).
- `export(colors, formats, Grouping, Document)` writes the reference's txt/json export (upper-case headings, `#` before HEX, heading stripped from values).
- The settings domain lives in `rikka_settings::session::color_picker` (formats list validated for unique non-empty names); rikka-settings itself never links this crate.
