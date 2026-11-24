---
title: Shell taskbar, tray and control centre
description: The per-output edge strip taskbar on any display edge, docking slides, tray, status and the control centre.
about: system/rikka-shell/crates/shell-bar
tags: [shell, taskbar]
---

- The taskbar is an edge strip plus a `BORDER`-thick frame on the other three edges, in the strip's background colour, whose `CORNER` overhangs round all four corners of the desktop inside. `rikka.appearance/taskbar` says whether it is up and `taskbar-position` (`TaskbarPosition`: top, bottom, left, right; default top) on which edge, for every display.
- `shell_kit::taskbar`: `Edge` (wraps the position) maps strip-local rects (x along the strip, y away from the display edge) to surface rects (`place`/`local`), radii (`radii`, local top = edge side) and insets; `Shape` is the geometry at an edge (`desktop(bounds)`, `covers`, `insets`, `strip` inset, `extent` = surface depth incl. room for the island, deeper for vertical edges). `TaskbarLayout::read`/`following(shell, |Option<Shape>|)` follows both keys and is the geometry source for backdrop, overview, desk, desk editor and control centre.
- Every edge has its own surface set (`feature.rs` `Docks`): the bar plus `frame::surfaces(edge)` (opposite edge with rounded corners, the two sides square), all bound to that edge's presence with `.animated` and `.after` the other three, so a change slides the old set off its edge (`rim::docking`, `SLIDE_OUT`), and the runtime destroys it and creates the new set in one turn (windows reflow once), which slides in (`SLIDE_IN`). Hiding slides out too.
- The frame relies on layer-shell arrangement order (creation order): bar, opposite, then the sides, so the sides span between them and no two surfaces paint the same pixel. `rim::paint` draws every band and corner overhang.
- `dock::Dock` (edge + output size) is the bar's orientation: `axis`, `length`, `surface`, `on_output` (bar-surface rect → output rect, for dropdowns and menus), `facing`. `src/axis.rs` holds the axis-generic layout helpers every bar widget uses (row or column, swapped sizes and paddings).
- Start of the strip (left or top): Nix snowflake (embedded SVG, accent `Mask`) toggles the assistant, then the workspace dots (`bar/workspaces::Slots`, fed by the island's `Tasks` board). Running apps live in the island (see the island and taskbar tasks notes); there is no dock.
- The centre of the strip is the island (`src/island/`, see its note); recording is shown and stopped there.
- The tray is a chevron over `rikka-tray`: shown only with items, with attention on the chevron and lazily fetched dbusmenus. Flyout and menus fade as one layer.
- Configurable indicators (input method, AI usage) sit between the tray and the status cluster; see the taskbar indicators note.
- Tray chevron, indicators and status form one gapless `strip::parts::cluster`. `src/cluster.rs` holds their shared metrics: every hover pill pads `PITCH / 2` sideways, so neighbouring glyphs sit on one `PITCH` across the whole cluster.
- Dropdowns (tray flyout, indicator panel) open from a `dropdown_panel::Opening` (item rect on the output + edge): below, above or beside the bar. The control centre sits in the desktop corner nearest the status cluster (top right, bottom right, bottom left for a left bar). Vertical bars stack the battery percentage under its glyph and point the tray chevron toward the desktop.
- `Daemons::start()` connects everything at shell start. Surfaces read `state()` once per frame after `changes()`.
- Any status glyph opens the one control centre: a stable tile grid, power, pages, notification history.
- 802.1X and pairing forms go to Settings. CoverTransition mode toggles every radio. Volume is optimistic.
- DND is a shared `Quiet` cell persisted in settings. No backlight or top-level sliders.
- The clock lives only in the island's idle face. The control-centre header's `clock` helper formats only battery time.
