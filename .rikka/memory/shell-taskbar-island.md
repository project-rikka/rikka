---
title: Taskbar island (clock or app icons in the strip)
description: bar/island: clock face or app icons in the strip, cards that swell out of it (media, peeks, window previews).
about: system/rikka-shell/crates/shell-bar/src/island
tags: [shell, taskbar]
---

- The island lives inside the taskbar surface, not in its own layer surface, so the strip and the island are one painted shape with no stacking races.
- The bar surface is `Shape.height()` tall (strip depth plus room below) with `PointerMode::Panes`: the input region is the exact rounded shapes of the frame's glass panes, so the pointer passes through everything but the strip and the drawn island.
- `compact::Mode` picks the face: `Idle` (no windows on the shown workspace) shows the live lead, then title / time / date and time, then the trail; `Tasks` shows the task list between equal fixed-width lead and trail containers (hover aims the live card), so icons never move sideways. The island grows with each app.
- Every change animates: both modes build one keyed structure (pill → row of lead, middle, the always-present `island.tasks` list, trail), each piece wrapped in `bar::glide::glide` (horizontal `AnimatedPosition` + enter/exit `Transition`, `grows()` or `settles()`), so pieces glide when the island recentres and leave as ghosts at their last place; `Pill` springs its drawn shape toward its laid-out bounds.
- `Body` is a full-surface, pointer-transparent column. Child 0 is `Face`; child 1 is a `Stage` (full-width, pointer-transparent, placed at the top) holding the card. Negative margins are not available (`Inset` clamps them), hence the stage.
- `Card::over` centres a card that starts at the top of the strip (media, peeks, idle clock card); the whole `Face` fades under it in both modes. `Card::below(left)` hangs a card under the strip at an x (window previews) and leaves the face visible; it must not cover the strip, or it takes the pointer from the task list.
- `Body` springs the swell below the strip (`motion::EMERGE` opening, `RESHAPE` closing), draws concave fillets, glasses the swell and clips children to strip-plus-swell. The compositor joins strip and swell panes into one glass body.
- The 2nd and 3rd live activities are round bubbles right of the pill; a spacer of the same width on the left keeps the pill centred.
- Cover art is never a child widget: face and card reserve an empty seat (`CoverFrame::seat()`) and report a `CoverTarget`; `CoverTransition` (painted by `Body` in `ctx.topmost`) flies one cover between perches; `seated` says whether the face perch is visible. Face offsets include the bubble mirror and the mode's lead inset.
- `Island` (nested `#[state]` of `Bar`, one per output) owns `Tasks`, the menu handle, activities, the latest moment (`peek`), the forecast, and hover: `aim` (`Target::Live` from the idle pill or a seat, `Target::Task(Aim)` from the list) and `showing`. A new aim opens after `OPEN_AFTER` (live) or `PREVIEW_AFTER` (tasks), switches at once while a card shows, and closes after `CLOSE_AFTER`.
- It always holds a minute `Need` on the shell `WallClock` and a second `Need` while recording or while the media card shows a playing timeline (`pace()`).
- `live::ranked` orders what is going on: playing media, recording, screen, camera, microphone, low battery, charging. A paused player shows only in the opened card.
- `notice::Notice::from_event` maps every `ActivityEvent` except track and playback; `shows_notice` filters what interrupts. A peek (incl. the track-change card) uses the same card mechanism.
- Live card (`is_open`, needs a player or recording): media (or the idle clock with weather and `ATTRIBUTION`), then rows for recording (stop), sensors and battery.
- Each `Island` holds an `Rc<Spectrum>` while a player plays; `AudioSpectrum` keeps only a `Weak` so every bar shares one capture. Dates come from `shell.island.*`; times are `%H:%M`.
- Animations only advance on rendered frames and a non-resting spring jumps across a long gap, so a frame-by-frame render check must keep rendering every ~16 ms until springs rest.
