---
title: Compositor selection crate (clipboard, primary, data control, drag and drop)
description: rikka-compositor-selection owns per-seat selections, compositor sources, ext-data-control, DnD and toplevel drags.
about: crates/compositor/rikka-compositor-selection
tags: [compositor, wayland]
---

- The host implements `SelectionHost: SeatHost + ShellHost` (`selections()`, defaulted `trusts_selection`, `send_selection`, `selection_event`) and installs `SelectionProtocols`.
- The host must forward every `SeatEvent` to `rikka_compositor_selection::seat_event`; keyboard focus changes re-send clipboard and primary offers to the newly focused client.
- Globals: `wl_data_device_manager` v4, `zwp_primary_selection_device_manager_v1`, and privileged-only `ext_data_control_manager_v1`.
- One `Selections` holds per-seat clipboard and primary slots, every device, all sources (client and compositor) in one slotmap, and DnD offer records.
- Sources are `Source::Client(ClientId)` or `Source::Compositor(CompositorSource)`; read the current one with `Selections::current(seat, kind)`.
- Mime lists are `rikka_compositor_types::MimeList = Rc<[Box<str>]>`, frozen once a source is used: client sources collect `offer`s until their first `set_selection`/`start_drag`; later wl_data_source and primary offers append to a new frozen list, data-control ones are `invalid_offer`. `set_compositor_selection` and `start_compositor_drag` take a `MimeList`; `Current` and `ForeignOfferInfo` hand out `&MimeList` so the X bridge shares it with xwm without copying.
- Commands: `set_compositor_selection` (the host later gets `SendSelection` with the fd), `clear_selection`, `request_data` (reads from whichever source is current).
- Commands and compositor/foreign drags take any `SelectionAccess`: the host, or `SelectionParts::new(&mut selections, &mut shell, SeatParts, &foreign_drag_target)`, which queues `SelectionEvent`s, `SendSelection` and `SendDrag` in `Selections` until `rikka_compositor_selection::flush(host)`, and drives the seat through its `SeatParts`. Admission (`trusts_selection`), drag start and icon roles stay on the host path.
- Client `set_selection` is admitted when the host `trusts_selection` (rikka-nested trusts its one client), else it needs keyboard focus, a serial not older than the focus enter or the current selection, and not from the future; refused sources get `cancelled`.
- Setting the source that is already current again re-announces it with its added offers and reports `Changed` (some toolkits do this after appending `SAVE_TARGETS`); a used source that is no longer current is `used_source` on wl_data_device and ignored on primary.
- X11 bridge: xwm `SelectionOffered` → `set_compositor_selection`; its `SendSelection` → `xwm.request_selection`; xwm `SendSelection` → `request_data`; `SelectionEvent::Changed` → `xwm.set_selection` unless the current source is the bridge's own.
- A drag only follows an xdg toplevel if its source had an `xdg_toplevel_drag_v1` when the drag started (`Shell::toplevel_drag_started` returns whether it did). A surface that is already the icon of an active drag cannot become another drag's icon.
- DnD runs as a `PointerGrab` or, for touch, a `TouchGrab` following one touch slot (drop on its up, cancel on touch cancel). All drag state lives in `Selections`, and ending the drag's grab from anywhere cancels the drag (`end_drag_grab`).
- `start_drag` is validated with `Pointer::grab_start(serial)`, then `Touch::grab_start(serial)`; invalid requests cancel the source. `DragView` carries the driving `DragDevice` and its location `at`; the icon gets the `DragIcon` role and its `wl_surface.offset` accumulates into `DragView::icon_offset` (draw at `at` + offset).
- Action negotiation follows weston: legacy (<v3) peers imply copy, ask is settled after the drop, and a destroyed dropped offer finishes a legacy target or cancels a v3 one.
- A source with an `xdg_toplevel_drag_v1` attachment emits `DragToplevelMoved { at }` (window geometry origin) per motion and `DragToplevelReleased` at the end; the host moves the window and must exclude it from hit-testing.
- Tests use a threaded display with seat, xdg shell and this module; `tests/common/client.rs` logs device and source events as strings.
- Source feedback goes through `drag::feedback::Feedback`: client sources get `wl_data_source` events, compositor drag sources get `SelectionEvent::CompositorDrag`.
- A release with an attached toplevel sends `dnd_drop_performed` before `cancelled`, so a client can tell a window let go of in the open from an abort.
- Foreign targets: when `SelectionHost::foreign_drag_target(surface)` says so (X11 surfaces), the crate emits `ForeignDrag` events and the host answers with `foreign_drag_status`, `foreign_drag_data` and `foreign_drop_finished`.
- Compositor-driven drags (`start_compositor_drag`, `move_compositor_drag`, `drop_compositor_drag`, `cancel_compositor_drag`) have no grab; data requests reach the host as `send_drag`. They never target foreign surfaces, and their source retires once nothing uses it.
