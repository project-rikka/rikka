---
title: Scrolling column layout (rikka-compositor-scrolling)
description: Strip<W> columns on an endless horizontal strip, view fitting, stacking, swipes; how the compositor drives it.
about: crates/compositor/rikka-compositor-scrolling
tags: [compositor, windowing]
---

- `rikka.windows/layout = "tiling"` with the `scrolling` flag on gives each display workspace one `Strip<WindowId>` instead of a dwindle tree; the compositor folds the two keys into `LayoutMode::Scrolling`. The pure crate never sees Wayland and is tested through its public API.
- A `Strip` lives in a `Frame` (work area, full output, `Spacing { inner, outer }` from the gap settings); call `reframe` when the area changes and `settle` before `tiles`, which places every window, off-screen ones included.
- The view is kept as an offset from the active column, so columns added or removed left of it leave it in place. `settle` scrolls only as far as needed to show the active column whole (Niri's `never` centring) and does nothing while a resize or swipe holds the strip.
- `insert` opens a column right of `next_to` (else the active one) at half width without resizing others; removing a column just opened returns to the one it came from with its old view.
- Columns stack windows by weight; `consume_or_expel(Side)` joins a lone window to its neighbour or takes a stacked one into its own column. `shift`, `focus`, `swap`, `drop_at`, `cycle_width` (⅓, ½, ⅔), edge `resize` + `release` and `enlarge(Sizing)` (an enlarged window gets its own column filling the output) complete the API.
- `swipe_begin/update/end` follow the fingers (one work-area width per 1200 units), then snap to the nearest column edge from the projected fling and return the view speed for the glide.
- In the compositor, `windows/tiling` holds a per-workspace `Layout::{Dwindle, Columns}` chosen by `Layout::from_mode(mode)`; `windows/columns.rs` runs the `Group::Columns` shortcuts (inert outside scrolling), `strip_swipe.rs` takes horizontal 3-finger swipes over a strip when no overview shows.
- Scrolling is click-to-focus; any keyboard focus change calls `scroll_to`, and `refocus` prefers the strip's active window. Strip windows are `confined` in `Desktop` so they never draw on a neighbouring display, and surfaces wholly off their output are occluded, so they get frame callbacks only at the `UNSEEN` pace.
