---
title: Shell overview
description: Super+Tab / 3-finger swipe overview: live compositor-drawn windows, card strip, thumbnails, drag, gestures.
about: system/rikka-shell/crates/shell-overview
tags: [shell, windowing]
---

- One exclusive-keyboard overlay per output (`rikka.overview`, namespace `surfaces::OVERVIEW`); the compositor hides that display's windows while it is mapped, so the overview draws them itself as window views.
- Windows are never captured: the deck paints `ctx.draw_window(WindowView)` and the compositor draws each window's live buffers there (see `rikka_window_view_v1` in compositor-windowing-protocols). Views sit above the whole surface, so any chrome must stay outside a window; card and thumbnail rings do.
- Windows carry no chrome (no plate, ring or close button): the one emphasis is a spring scale-up of the lifted window (`Deck::lifted`). The last input decides it (`Lead`): the pointer lifts the hovered window, keyboard navigation lifts the selection, and Enter/Delete act on the lifted window.
- Look: no backdrop of its own: a `DesktopFrost` (Bottom-layer scrim) frosts the whole desktop under the taskbar, its opacity set to the spread by `Deck::draw` (`Shared::frost`, `Board::frost`); a centred "card" per listed workspace drawing the unblurred wallpaper, neighbours peeking left/right; a thumbnail row on top (wallpaper + live windows, current ringed). No titles or icons.
- Listed workspaces = occupied ∪ shown ∪ the next empty one after them (`stage::listed`).
- `stage.rs` is the only session reader and builds a `deck::Board`; `deck/frame.rs` is pure geometry (stage below the taskbar, card at spread, thumbnails); `deck/strip.rs` drives spread and strip motions; `deck/paint.rs` draws; `deck/input.rs` hit-tests `Hits` recorded by the last paint.
- Window placement is the natural layout in the shell-private crate `system/rikka-shell/crates/overview-layout` (`arrange`, tested for shape, containment and spacing). Slots are kept in card-relative units and glide per window id.
- Spread 0 is pixel-identical to the desktop (card = output, windows at real rects, stage below the taskbar), so opening and closing are seamless. Exit is a `EXIT_DURATION` tween so `animated` keeps the surface long enough.
- The taskbar stays visible and clickable during the overview (pointer `Reach` inset by its reserved depth); the stage and layout sit below it.
- 3-finger swipes come from the compositor over `rikka.gestures`. The feature-level `swipe::Swipes` handle drives visuals (deck repaints through its `Watchers`) and opens/closes (vertical); each `Stage` keeps its own `Swipes` to decide horizontal workspace steps on its output. Both use `swipe::step`/`Track::stays_open`, so visuals and decisions agree.
- Other input: click window activates, click empty card/backdrop closes, click neighbour card or thumbnail switches, drag a window onto a card or thumbnail moves it (pinned at the drop until the move arrives), middle click/Delete closes, wheel and horizontal touchpad scroll switch, arrows/Tab select (the first key press lifts the hovered or chosen window and the next ones move), PageUp/PageDown and Ctrl+arrows step, digits show a workspace.
- The card wallpaper comes from `shell_wallpaper::shared` first (the backdrop's decoded picture or video controller), so the first frame already has it and videos stay frame-identical.
- Desk widgets ride on every card: the deck hosts one `framed` scene per listed workspace as children and publishes each card rect in `deck/cards.rs` during its paint, so the scenes follow in the same frame; deck chrome is drawn `topmost` above them.
