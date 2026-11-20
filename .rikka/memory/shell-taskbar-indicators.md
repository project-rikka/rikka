---
title: Taskbar indicators (input method, AI usage)
description: bar/indicators: the Indicator trait, the Shelf registry, one shared panel surface, and how to add an indicator.
about: system/rikka-shell/crates/shell-bar/src/indicators
tags: [shell, taskbar]
---

- Indicators sit between the tray chevron and the status cluster. Each draws a mark in the strip and a body in one shared overlay panel (`Drawer`, surface `indicator`).
- `Indicator` trait: `kind`, `changes` (a `Send` stream that re-views strip and panel), `mark` (`None` hides it), `panel(&Shelf)`, and optional `opened`/`tick` hooks.
- `opened` and `tick` run inside a bar's handler, so they may `spawn`. Each bar ticks every 30 s (`every_keyed` from `Bar::start`) and once at start; models dedupe by age.
- `Shelf` owns the registry, which kinds are shown, which is open, its anchor point, `close()` and `launch(argv)`. Panel handlers are plain closures over `Rc` models.
- `rikka.appearance/taskbar-indicators` is a list of slugs. `session::appearance::Indicator` (`ALL`, `of`, `slug`, `shown`, `written`) is the catalog; the shell aliases it as `Kind`.
- Adding one: a session `Indicator` variant and slug, a module implementing the trait, one entry in `indicators::all`, a Settings label in `INDICATORS` (Appearance page), and locale strings.
- `shell-bar/src/dropdown_panel.rs` is the one panel chrome hung off a point (tray flyout and indicator panel) and owns the shared row metrics. `parts.rs` holds indicator panel blocks (`heading`, `rule`, `entry`). `shell_kit::highlight_button` is the hover-highlighted pressable that reports its bounds; the tray chevron sits on it.
- `Typing` reads `shell_ime::InputMethods`, the shared handle the IME feature fills with its `Panel`. Opening it triggers `/Fcitx/im` so fcitx answers with the group's input methods.
- `Usage` holds its own `Llm`, follows `rikka.llm` through `AccountsChanged::from_delivery`, and meters every held metered account with `accounts::Reading`. The mark is only the `AUTO_AWESOME` symbol; usage figures appear in the panel alone.
