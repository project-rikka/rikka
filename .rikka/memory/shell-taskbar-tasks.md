---
title: Taskbar tasks (running apps in the island, the dock's replacement)
description: bar/tasks + task-board crate: the shown workspace's running apps as icons, click rules, previews, right-click menu.
about: system/rikka-shell/crates/shell-bar/src/tasks
tags: [shell, taskbar]
---

- There is no dock and no pinning; launching is the launcher's job. The island's `Tasks` (`tasks/state.rs`) is the only list of running windows.
- `system/rikka-shell/crates/task-board` (shell-private, tested) is the pure model: `Board::arrange(toplevels, output, showing, &mut Order, owner)` gives `COUNT` `Space`s of `Group`s (one per app key, or per window without an app id) of `Window`s; `Order` keeps arrival order so restacking never reshuffles; `Board::looks_like` ignores titles and bounds.
- `Group::press` is the click rule: one window → focus, or minimize when focused (nothing when tiled); several → cycle to the next after the focused one, else the topmost non-minimized (`recent`).
- `tasks::Tasks` (held by `Island`) follows `rikka.toplevels` and `rikka.workspaces` for its output, resolves a `Tile` (name, `Art` icon via `AppIcons::for_app`, desktop-entry command) per group key through `apps::owner`, and sends focus/minimize/close/show/launch requests. `shown()` is the shown workspace's groups; `holding()` is the compositor's per-output holding that feeds the left workspace dots (`bar/workspaces::Slots`), so the bar keeps one subscription.
- `task_list(TaskList { groups, space, tiles, longest, axis }, on_touch)` builds a line of `TaskIcon` widgets along the bar's axis (one per group of the shown workspace, keyed by space and key, each in `glide` with `grows()`): icons only, no running/focus marks; minimized groups dim; hover lifts. Cells squeeze to fit the bar length minus two flanks.
- Input: primary click on release (`Group::press`), middle starts a new window, secondary opens the menu. Icons report their bounds (`Aim::bounds`, `Touch::Chose::bounds`); the island turns them into the edge-facing point on the output (`Dock::facing`). Entering an icon reports `Touch::Aimed`; in idle mode hovering the pill aims the live card (`Island::hover_face`).
- Previews (`island/details/preview.rs`): one entry per window with title, close button and a live `thumbnail` (a `WindowView` via `rikka_window_view_v1`), in a row on top/bottom bars and a column on side bars; clicking focuses and closes the card.
- The menu is its own destined overlay surface `taskbar-menu` (`tasks::menu`): window list (several), new window, maximize/restore and minimize (single window), move (all) to workspace submenu, close (all). It opens away from the bar (`MenuRequest::toward` → `popover(..).opening(at, AnchorEdge)`).
- `Art` paints icons as tiles in the composer/card look (`palette.surface` fill, `outline` border; masks/glyphs in the accent, pictures in their colours, scaled by `AppIcons::footprint`).
