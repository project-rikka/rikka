---
title: Internationalization
description: rikka-i18n-catalog parses catalogs and messages; rikka-i18n loads, negotiates and installs; catalogs!() checks catalogs.
about: crates/foundation/rikka-i18n
tags: [i18n, foundation]
---

- `Locale::system()` follows the message category (`rikka_types::LocaleCategory::Messages`: `LC_ALL`, `LC_MESSAGES`, `LANG`).
- `rikka-i18n-catalog` owns the format: `Catalog`, `Message`, `Locale`, `Args`, plurals, `Error` and `verify_catalogs`. `rikka-i18n` owns the runtime (builder, globals, `load_dir`, `Label`) and re-exports the catalog crate.
- `rikka-i18n-macros` depends on `rikka-i18n-catalog`, not `rikka-i18n`, which depends on the macros.
- Catalogs are TOML with `[i18n]` metadata. Nested tables flatten to dotted keys. For a crate with `locales/`, the `rikka_*` rules of `//bazel/rust:defs.bzl` set `RIKKA_LOCALES` to its catalog file names and add them as compile data; `rikka_i18n::embedded!()` embeds them all and falls back to `en`, so a new catalog file needs no code.
- A library crate with `locales/` exports `pub static CATALOGS = rikka_i18n::catalogs!()`; each app adds it with `embedded!().embedded(CATALOGS)` (rikka-effects does).
- `catalogs!()` runs `verify_catalogs` while it expands: each file is named `<locale>.toml`, `en.toml` exists and is non-empty, and every catalog has exactly its keys. A mismatch is a `compile_error!`.
- Messages support substitution plus plural, ordinal and select (`other` is mandatory). A missing key renders itself, and a missing argument stays visible.
- `I18n::builder()` embeds catalogs, loads user dirs and sets the fallback. Globals: `tr`, `tr_with`, `set_locale` (pin), `set_language` (negotiate), `generation`.
- Fallback widens subtags and pluralizes by the fallback message's language.
- Bad user catalogs are skipped into `LoadReport`.
- A language change bumps one generation, which rebuilds translated views.
- No number, date or currency formatting.
- `rikka-languages` (public) is the one language table on BCP-47 tags: `Language::{all, from_tag (exact), lookup (RFC 4647 fallback plus deprecated and service aliases: iw→he, tl→fil, no→nb, zh-TW→zh-Hant…), tag, english_name, native_name}`. The browser's page translation and the shell's translate window both use it; services map tags to their own codes at their boundary (`browser_translate::Service::aliases`).
