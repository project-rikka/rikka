---
title: Shell keyboard, IME, polkit and notifications
description: On-screen keyboard, fcitx5 candidates, polkit prompts and notification cards with history.
about: system/rikka-shell/crates
tags: [shell, services]
---

- On-screen keyboard: a non-focusing overlay (`rikka.keyboard`, `Super+K`) that uploads a US ANSI keymap through `rikka-virtual-input`, latches modifiers, locks Caps, and releases held keys on exit.
- IME: starts and replaces fcitx5 and takes the kimpanel name before drawing. One pointer-ignoring, panel-sized candidate surface per output is `positioned` beside the caret, then the kimpanel spot, then a fallback; it lives as long as the shell and is only unmapped between compositions. Its Overlay namespace `surfaces::INPUT_PANEL` makes the compositor keep it above overlays created later (launcher, toasts). No current-source state is duplicated.
- The candidate panel pops in from its caret-side corner (`shell-ime/src/showing.rs`, `Positioner::is_flipped`); when a composition ends, `Window` keeps the last one shown for `POP_OUT` before unperching, so it pops out too. `shell-ime/src/candidates.rs` paints the candidates itself so the highlight glides by `INDICATOR` and the text inks follow it.
- Polkit: `daemons::polkit::Agent` starts at shell start and shows exclusive modal prompts per `Ask`. Passwords stay in-process, never in seam payloads, and serials clear stale secrets.
- Notifications own `org.freedesktop.Notifications` via `rikka-notifications` and draw a top-right stack.
- The toast surface is a full-height column flush with the right screen edge (`view::SPAN`: leading `OVERSHOOT` room, cards, trailing `MARGIN`) with `PointerMode::Panes`, so input lands only on the glass cards and no animation ever leaves the surface. Each toast is a keyed `dismissible` slot (`arriving`, `dismissed = leaving`); the queued pill fades.
- The service holds cards and queue. Surfaces own only generation-tagged expiry and exit timers; a leaving toast is removed `Dismissible::DEPARTURE` after it starts leaving. Hover pauses the stack, and overflow waits.
- Expiry and a swipe put a toast away into history (transient ones close as expired or dismissed); the close button dismisses it, and a click runs its default action or else dismisses it.
- The centre's history list renders `HistoryList` rows: history entries are `Rc`s diffed by pointer, so rows that left history stay as `leaving` dismissible slots until `DEPARTURE`, and rows added while it is open arrive. Rows swipe away to `forget`; the empty tile opens and closes in a `collapsible`.
- Do-not-disturb sends non-urgent cards to history. Expired and suppressed cards stay actionable there. Handled, transient and dismissed cards leave.
- A notification stays open on D-Bus until it leaves both stack and history.
- App identity resolves once through desktop entries and icons. History is bounded and memory-only (`History`).
- `ActionRouter` owns card answers: per notification a map of action key → `ActionRequest`, run as local seam calls instead of D-Bus actions. An `Announcement` may carry a default `action_request`, `button`s, `resident`, `progress` and a `tag`: a later announcement with the same tag replaces the open notification, and one carrying progress is dropped once the person closed it.
- `Notification.progress` (`Progress::{Indeterminate, Percent}`, also read from the spec's `value` hint) draws a bar on the card. A replacement with progress updates in place without re-alerting: `Toast::refresh` keeps the timer, `History::refresh` swaps the entry, and `HistoryList` matches rows by id so they neither leave nor arrive.
