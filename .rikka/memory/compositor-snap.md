---
title: Window snapping
description: Floating windows snap to edges and snap-bar zones, resize together along shared lines, and get snap assist.
about: system/rikka-compositor/src/snap
tags: [compositor, windowing]
---

- Active only while `rikka.windows/layout` is `floating`. Settings `snap`, `snap-bar`, `snap-assist` and `snap-resize-together` default on (`SnapRules`).
- `rikka-compositor-snap` holds the pure parts. `layout.rs` is the zone template table (4 layouts landscape, 6 from 1920 px, stacked portrait). `at_edge` measures edges against the usable area, so top-edge maximize fires just past the taskbar.
- `bar.rs`: the bar peeks, opens and hides (`Reveal`). `state.rs` keeps one `Aim` per drag.
- Zones use the usable area. Snapped windows get all four tiled states and `Mode::Snapped` (zone, work area, prior shape, from `windows/mode.rs`), the source of occupancy and restore; an enlarged snapped window keeps its `Snapped` inside `Enlarged` and returns to it.
- `divider.rs` moves every window on a shared line (8 px grab, or dragging an inner edge). The group is the topmost snapped window of each region (`snap_group`); a divider press counts only where the hit is a member, a gap or a Background/Bottom layer.
- `paint.rs` draws the bar, preview and assist. Colours are only in `element.rs` (background, surface, surface_variant, selection roles); Staging passes the palette in directly.
- The assist's empty zones are always frosted with the scrim blur, whatever the session's glass (`Assist::frost` keys, `Staging::frosted`, `Composition::frost_over` beneath `over`).
- Assist (`assist.rs`) flies the other windows live into cards (`assist::slots`, one row that breaks only when cards get too narrow). Escape, a missed press, a workspace switch or an unmap dismisses it. Cards are lifted above everything only while it is open; dismissed cards fly home at their own stack depth.
- `snap.advance()` runs once per frame.
