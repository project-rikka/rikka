---
title: Virtual input client (rikka-virtual-input)
description: Seat, Pointer, TextKeyboard, caps Keyboard and PhysicalKeyboard over virtual pointer/keyboard; threads, units, drop.
about: crates/session/rikka-virtual-input
tags: [input, session]
---

- `Seat::connect|connect_to(socket)|named(seat)` opens its own Wayland connection with a `Host::with_reader` reader thread and binds its `wl_seat` once; every device gets a clone of that proxy and runs on its own `rikka-*` worker thread, so calls only enqueue (`TextKeyboard::set_modifiers` and `Pointer::sync` are the only waits). Workers never read: where they need the server to have caught up (output names, the seat keymap, keymap changes, teardown) they `EventLoop::settle`.
- `pointer()` and `pointer_on_output(name)` (wl_output v4 name, `zwlr_virtual_pointer_manager_v1` v2 `create_virtual_pointer_with_output`; `Error::OutputNotFound`) give a `Pointer`: `move_to(AbsolutePosition)` maps x/x_extent onto that output (else the pointed one), `move_by(dx, dy)` is relative logical px (cursor moves, relative-pointer clients and locked pointers get it), `press`/`release` act at the current spot and ignore repeats.
- Scroll signs follow Wayland (positive = right/down): `scroll(notches)` and `scroll_value120(x120, y120)` are wheel source, 15 px per notch, with `axis_discrete` only when a 120 step accumulates (the protocol carries whole notches only); `scroll_continuous(dx, dy)` is continuous source in px, no stop.
- The pointer worker drains its whole queue per wakeup, ends each event with `frame` and flushes once; adjacent motions and scrolls coalesce while it is busy.
- `text_keyboard()` types text and chords through generated keymaps; a chord presses its modifier keys (left Ctrl/Shift/Alt/Super) as real key events around the key and releases them in reverse, so clients see modifier releases as keys, not only as `modifiers` state. `keyboard(caps)` is the shell on-screen keyboard's own keymap.
- `physical_keyboard()` types evdev `KeyCode`s with the seat's keymap: it binds its own `wl_keyboard`, forwards each keymap fd it is sent to the virtual keyboard (identical, so the compositor's intern sends clients nothing new), and never sends `modifiers` (the compositor derives Shift/AltGr/Caps from keys). It is an `EventLoop::run` worker that uploads each keymap from its `wl_keyboard` handler, ignores repeated presses, and `release_all()` or drop releases every held key.
- Drop of any device stops its worker, which releases held keys and buttons, destroys the object and flushes (settling when pointer buttons were held).
- Tested through the compositor harness in `system/rikka-compositor/tests/virtual_input.rs`; the crate has no tests of its own.
