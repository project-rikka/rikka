---
title: Tabs, split panes and cross-window tab drag
description: split/pane divide a window; tabs travel between one process's windows over Wayland drag-and-drop.
about: crates/ui/rikka-widgets/src/navigation
tags: [ui, drag]
---

- `split(axis, ratio, first, second).on_resize(..)`: the ratio is the only number a caller keeps. `ratio` is `impl Into<SplitRatios>`: a plain `f32`, or a shared `Ratio` (`Observed`) that the split and `CaptionBar::columns` shares (`ratio.first()`, `.second()`, `Share::within`) read at layout time, so `Ratio::set` relayouts them with no view. The gutter is 6 px, each side keeps at least 56 px, and hovering sets a resize cursor.
- A strip that moves tabs takes `tabs(..).transfers(Transfer::group(name).take(..).insert(..))`, optionally `.tear_off(handler, tears_off_last)`; a strip that only receives takes `tabs(..).accepting(name, land)`. `on_settled` stays on the strip. `tabs/transfer.rs` owns `Transfer` and the MIME, `tabs/drag.rs` the drag events. The offered MIME is `application/x-rikka-tab;group=<g>;pid=<pid>`, so tabs move only within one process and group.
- `Transfer<T>` carries the app's live tab type: `take(|s, index| -> Option<T>)`, `insert(|s, at, tab: T|)`, `tear_off(|s, tab: T|, last)`, and `accepting(name, |s, at, tab: T|)`. Live tabs are never serialized: the strip moves them through rikka-ui's thread-local `hand_over`/`take_over`, which apps never call.
- Events: `DragTaken` asks the origin for the tab, `DragDropped` gives it to the target, `DragEnded` closes the drag. The origin hears `DragEnded` on delivery, so an emptied window closes before it paints.
- The source removes a tab only after acceptance. A tab nobody took returns to its index, and `Aborted` changes nothing.
- Tab drags ask to move windows. `rikka-app` sends the origin `DragWithin(bool)`: whether the drag is over any of this process's windows, decided only after the session first reports where the drag is. While within, a tab never tears off, and a drop nobody takes there ends `Aborted`, so the tab returns.
- On `DragWithin(false)` the strip lifts the tab and asks `DragWindow::Opening`; `rikka-app` attaches the new window through xdg_toplevel_drag but withholds its frames (`Windows::withhold`), so only the tab ghost travels and the window maps where the drag lets go.
- A lone tab (one tab, no `tear_off(.., true)`) asks `DragWindow::This` as it is lifted, so its whole window follows the pointer at once and still shows the tab. The compositor draws a carried window see-through while the drag's target accepts it (`DragView::accepted`, `SelectionEvent::DragAccepted`), so the strip underneath shows its gap.
- The compositor keeps an attached, still unmapped toplevel's arrival plan on the pointer (`follow_arrival`), and in tiling mode re-plans it as a tile on release (`land_arrival`).
- When the drag comes back within the app, `rikka-app` sends `DragReclaimed` to the torn window, which lifts its tab back and closes unseen. `Aborted` (Escape) reclaims too, and the origin re-lands the tab; `Landed`/`Nowhere` release the withheld window.
- xdg_toplevel_drag holds one attached toplevel until it is destroyed or unmapped; attaching another first is a fatal protocol error. `rikka-app` keeps a returned torn window as `Following::Returned` until it is gone and attaches nothing meanwhile, so the app's `on_settled` must close an emptied window with `request_window(WindowRequest::Close)`.
- Without toplevel-drag support, `DragEnd::Nowhere` tears the tab into a new window, except a strip's last tab unless `tear_off(.., true)`.
- A strip under a drag opens a gap slot and hides the dragged tab.
- The platform side is `EventCtx::start_drag`/`accept_drag`/`carry_window`. `rikka-app` runs its `wayland/data_device` and paints the ghost into the compositor's drag-icon surface; the ghost hides only while the origin window itself follows. The ghost owns its own `Resources`, so a `DragIcon` may draw pictures by path; they are released when the drag ends.
- `pane(content).accepting(name, land)` takes the same drags, `land` getting the `DropTarget` and the tab. `DropTarget::Split(Side)` means within a quarter of an edge, `DropTarget::Into` otherwise. A `Highlight` child tints the target half without taking events.
- A pane claims the keyboard via `Widget::claims_focus`.
