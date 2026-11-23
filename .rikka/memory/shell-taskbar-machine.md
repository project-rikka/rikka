---
title: Shell taskbar, tray and control centre
description: The per-output edge strip taskbar, workspaces, tray, recording stop, machine status and the shared control centre.
about: system/rikka-shell/crates/shell-bar
tags: [shell, taskbar]
---

- The taskbar has one fixed design: a top edge strip plus a `BORDER`-thick frame on the other three edges, in the strip's background colour, whose `CORNER` overhangs round all four corners of the desktop inside. There is no design setting.
- `src/strip/` holds the strip: `view` stacks the bar row over the island, `parts.rs` holds its pieces. `src/frame/` holds the bottom, left and right frame surfaces (Top layer, exclusive `BORDER`, pointer ignored, same presence as the strip).
- The frame relies on layer-shell arrangement order: the strip is registered first, then bottom, then left and right, so the sides span between strip and bottom and no two surfaces paint the same pixel (translucent backgrounds would double).
- `shell_kit::taskbar::Shape` is the fixed geometry (reserved depth, border, corner, `desktop(bounds)` = the rounded desktop inside the frame, `covers`, the taller strip surface that leaves room for the island, `PointerMode::Panes`).
- Only `rikka.appearance/taskbar` (up or down) moves strip and frame together. `TaskbarVisibility::read`/`following` report that, and `TaskbarVisibility::shape()` is `Some(Shape)` while up.
- `TaskbarVisibility` is the geometry source for the backdrop, overview, desk, desk editor and control centre; `shell_wallpaper::stage::desktop` is their shared desktop rect.
- Left: Nix snowflake (embedded SVG, accent `Mask`) toggles the assistant, then the workspace dots (`bar/workspaces::Slots`, fed by the island's `Tasks` board). Running apps live in the island (see the island and taskbar tasks notes); there is no dock.
- The centre of the strip is the island (`src/island/`, see its note); recording is shown and stopped there.
- The tray is a chevron over `rikka-tray`: shown only with items, with attention on the chevron and lazily fetched dbusmenus. Flyout and menus fade as one layer.
- Configurable indicators (input method, AI usage) sit between the tray and the status cluster; see the taskbar indicators note.
- Tray chevron, indicators and status form one gapless `strip::parts::cluster`. `src/cluster.rs` holds their shared metrics: every hover pill pads `PITCH / 2` sideways, so neighbouring glyphs sit on one `PITCH` across the whole cluster.
- `Daemons::start()` connects everything at shell start. Surfaces read `state()` once per frame after `changes()`.
- Any status glyph opens the one control centre: a stable tile grid, power, pages, notification history.
- 802.1X and pairing forms go to Settings. CoverTransition mode toggles every radio. Volume is optimistic.
- DND is a shared `Quiet` cell persisted in settings. No backlight or top-level sliders.
- The clock lives only in the island's idle face. The control-centre header's `clock` helper formats only battery time.
