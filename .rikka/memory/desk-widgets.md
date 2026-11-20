---
title: Desktop widget contents (desk/widgets)
description: The eight desk widget kinds: Live state, shared Sources, Body/card frame, ticks, needs, options and catalog keys.
about: system/rikka-shell/crates/shell-desk/src/widgets
tags: [shell, widgets]
---

- Contract for hosts: `Live::new(scenery)` (`.everything()` for the editor), `Live::start()` in the host's `on_start`, `widget(&Widget, &Live, Mode)` (exactly `widget.extent()`, draws its own pane ground), `options(&Widget, change)` for the inspector, `about(kind)` for the gallery.
- `Live` has only `#[untracked]` fields, so its handlers never re-view a host; data flows through observed `shell_kit::Feed<T>` cells in `Sources`.
- `Sources` is one `Rc` per UI thread (thread-local `Weak`), shared by every `Live` (backdrops, overview, editor): note drafts, calendar pages, monitor history and the spectrum hold are common to all surfaces.
- Each `Live` subscribes with keyed streams only to what `Needs::for_layout(board.shown_layout())` asks (weather, MPRIS, machine, battery, Bluetooth) and re-evaluates on every board announcement; turning a source off only cancels, never clears shared data.
- Time comes from the shell `WallClock` (`sources.feeds.wall`): bodies read `now/slot/follow(period)` and `today()`; each `Live` holds one `Need` at the smallest cadence its layout asks (`Needs::cadence`, seconds for any clock with seconds). The analog `Dial` reads time in paint and repaints through `wall.painting(period)`.
- `card()` wraps a `Body` widget whose build reads the feeds, so a source change rebuilds only that card; `Mode::Shown` makes the body `INERT`, so previews look identical and pointer events fall through to the host.
- Lines and hands are turned sprites of a 1×1 white texture (`paint::stroke`). `paint/` holds the theme-neutral data marks (accent fill, text-colour tracks): `span` (range bar, `.marked` dot), `sparkline` (`.gridded`), `ruler` (24 h day ruler, `.labelled`), `segments` (battery cells), `columns` (per-core bars) and `trend` (hourly curve).
- Geometry: a cell is 152 px with 12 px gaps and margin (`desk-layout` `widget.rs`/`area.rs`); cards pad by `card::INSET` (12) and round inner pictures with `card::INNER`.
- Shared look lives in `kit.rs`: `header(symbol, title)` is every widget's accent heading (`.trailing(label)` in monospace, `.beside(view)`, `.tint`), `figures()` sets live numbers in the monospace family, `display()` sets big light numerals.
- Media shows progress as `media/scrubber.rs`: live spectrum bars coloured by position that seek on press/drag; it shares `feeds::Levels` smoothing with the island's `waveform`. Any media widget asks `Needs` for the waveform and a one-second timeline.
- Photos derive the shown picture from wall time (`slot = unix / interval`, seeded shuffle per widget and round), so every surface shows the same photo; albums rescan every 10 min and stills decode off-thread at 2× the card size.
- Notes edit a shared `TextDocument` per widget id, write back to `board.showing()` 600 ms after typing or on blur, and are reset from the board only when not being typed in.
- Catalog keys live under `shell.desk.widget.*`; weekday names, sky symbols/conditions, `degrees` and `percent` are shared with the island through `shell-feeds`.
- Apps are started with `rikka_open::spawn_detached` (`rikka-task-manager`, `rikka-settings`); Settings has no page argument.
