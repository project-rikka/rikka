---
title: Shell taskbar, tray and control centre
description: The per-output edge strip taskbar, workspaces, tray, recording stop, machine status and the shared control centre.
about: system/rikka-shell/crates/shell-bar
tags: [shell, taskbar]
---

- The taskbar has one fixed design: an edge strip across the top whose corner overhang cuts the desktop below it. There is no design setting.
- `src/strip/` holds it: `view` stacks the bar row over the island, `parts.rs` holds its pieces, `Shape` is its fixed geometry (reserved depth, corner, the taller surface that leaves room for the island, `PointerMode::Panes`).
- Only `rikka.appearance/taskbar` (up or down) moves it. `TaskbarVisibility::read`/`following` report that, and `TaskbarVisibility::shape()` is `Some(Shape)` while up.
- `TaskbarVisibility` is the geometry source for the backdrop, overview and control centre.
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
