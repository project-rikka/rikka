---
title: Shell colour picker
description: rikka.color-picker: frozen-screen loupe picker per output, floating editor with history, shades, tuning, export.
about: system/rikka-shell/crates/shell-color-picker
tags: [shell, color]
---

- `Super+Shift+C` (`Shortcut::ColorPicker`) calls `rikka.color-picker/activate`, which picks or opens the editor per `rikka.color-picker/activation`.
- The service is the editor's `PresenceService` (`open/close/toggle`) plus `activate`, `pick`, `chose {color, button}` and `cancel`; the picker surfaces only notify it.
- `pick` freezes every output with capture's `Frozen` (no cursor) and shows one overlay per output; if the editor is up it hides it first and re-notifies `pick` from `Presence::after_unmap`, so the editor is never in the frozen frame.
- `picker/stage.rs` is one widget: frozen frame, hidden cursor, a loupe centred on the pointer (wheel or `+`/`-` zoom, arrows nudge one physical pixel, Shift ten) and a card with swatch, copied-format value and optional name. Buttons act on release; Enter/Space is the primary button, Escape cancels.
- The card (`picker/card.rs`) is a glass pane in `palette.surface` like the IME candidate window; the stage draws the frozen frame only around it, so the compositor's blur of the live desktop shows through the hole.
- The service maps the button through the settings (`Click`), copies `Preferences::copied().render` (`template_named` over the copied format), records history and may open the editor.
- History is a `shell_kit::Recents<Rgb>` in `$XDG_STATE_HOME/rikka/color-picker.json` (newest first, at most 20).
- The editor (`shell_kit::floating`) keeps a snapshot of history: multi-select (Ctrl/Shift) lazy column, right-click menu (remove, export by colour or format), shades bar, a `color_picker` popover to tune, and copy rows for shown formats. Delete removes, Ctrl+C copies the current colour.
- Export hides the editor, asks a save path through `rikka-dialog` on a thread, writes txt or json by extension, then reopens the editor.
- The gear opens Settings at `Section::ColorPicker` (`rikka_session::settings_app::Link`).
