---
title: Compositor seat crate (keyboard, pointer, touch, tablet, IME, idle)
description: rikka-compositor-seat owns wl_seat protocols, focus, grabs, constraints, text-input/IM bridge, virtual input, idle.
about: crates/compositor/rikka-compositor-seat
tags: [compositor, input]
---

- The host implements `SeatHost: SurfaceHost` (`seats()`, `display()`, `filter_key`, `seat_event`) and installs `SeatProtocols`; the Xwayland keyboard grab manager is published only to clients admitted with `ClientOptions::xwayland()`.
- The host must forward `SurfaceHost::committed`/`destroyed` to `rikka_compositor_seat::surface_committed`/`surface_destroyed`.
- `Seats::add(&display, name, Keymap)` publishes one `wl_seat` at the vendored XML version (v11) per seat; the first seat is `Seats::primary` (virtual pointers without a seat land there).
- Commands go through `rikka_compositor_seat::seat(access, id)` → `SeatMut` for any `SeatAccess`: the whole host, or `SeatParts::new(&mut seats, &mut surfaces, &display)` (borrowed stores; `into_seat(id)` gives an owning `SeatMut`). Every command (focus, keymaps, warp, constraints, grabs, tablets, IME, activity) works on both; only input delivery (`key`, `motion`, `button`, `axis`, `gesture`, `frame`, `touch_*`) needs the host, because it runs the key filter and the host's grabs. Reads go through `Seats::get(id)` → `&Seat`.
- With the host, reports reach `seat_event` (and grabs' `ended`) at once; through `SeatParts` they are queued in `Seats`, ended grabs included, until the host calls `rikka_compositor_seat::flush(host)`. Internals are generic over the sealed `Access` trait; only role assignment, the key path and grab dispatch stay `S: SeatHost`.
- The seat never hit-tests: the host passes `Motion.under`/`TouchDown.under` as `Focus { surface, origin }` and calls `frame()` itself, so input can be batched once per frame.
- Every key (device, virtual keyboard, IM) takes one path: host `filter_key` first, then the IM keyboard grab unless the source is the IM client's own virtual keyboard (`KeySource::InputMethod`), then the focused client.
- Each origin (all physical devices together, and each virtual keyboard) is an `XkbState` (`keyboard/xkb_state.rs`) with its own xkb state and pressed keys; a release goes only where its own press went, and an unmatched release is dropped. fcitx5 relies on this: it forwards and repeats keys through its own virtual keyboard.
- Clients and the IM grab each keep a `SentKeyboardState` (origin, keymap, modifiers last sent); keys routed to the grab never change what clients were sent, so while grabbing only the IM's virtual keyboard sends clients modifiers.
- Physical keyboards use the seat's keymap (`set_keymap`), a virtual keyboard uses its upload; a destination is shown the keymap of the origin that last typed to it, so an identical upload never re-sends.
- Modifiers, locks and layout group of a virtual keyboard are derived by the compositor from its key events in its own `XkbState`; a client never needs `modifiers`, and lock state (Caps, Num) is per origin.
- Keymaps are interned per thread: every constructor returns the live `Keymap` whose compiled text (or last alternative spelling) matches, so an identical upload skips compilation, and `Keymap` equality is pointer identity.
- Keymaps are shared as one sealed memfd; each `wl_keyboard` remembers the stamp it was sent and gets the current one lazily on enter.
- A `wl_keyboard` out of focus is sent the seat's own keymap (devices' `default`) when created and whenever `set_keymap` changes it, so an unfocused client (rikka-virtual-input's `PhysicalKeyboard`) can follow and re-upload it.
- Modifier masks are resolved when a keymap is built; each `XkbState` caches its serialized and decoded `Modifiers`, so `Keyboard::modifiers()` is a field read.
- `Keyboard::held_modifiers()` is the union over every origin. Modifier changes that come without a keystroke (a virtual keyboard's `modifiers` request, or a destroyed virtual keyboard's keys being released) announce `SeatEvent::Modifiers { seat, held }`; the compositor's switcher ends a held Alt+Tab on it.
- Protocol objects are kept per client (`rikka_compositor_server::ByClient`), and pointer, keyboard and tablet-tool focus cache the focused `WlSurface`, so delivery never scans every resource or re-resolves the surface.
- Pointer and touch grabs are `PointerGrab<S>` / `TouchGrab<S>` trait objects kept in one type-erased `GrabSlot` each, which hands the same allocation back after each dispatch; defaults forward through `PointerHandle` / `TouchHandle`. Move/resize/popup/DnD grabs belong to their owners, not this crate.
- Pressed buttons form an implicit grab (`Pointer::grab_start(serial)`); each touch point keeps the surface it went down on (`Touch::grab_start(down serial)`). `GrabFocus::Clear` on a touch grab sends `wl_touch.cancel`.
- `TouchMotion.under` is the host's hit test; normal delivery ignores it, grabs such as touch DnD use it.
- Constraints activate only on the focused surface inside their region; the host clamps motion with `SeatMut::constrain` before hit-testing. Confinement clip rects are rebuilt when the surface commits a new region or size, never per motion.
- A device motion that changes pointer focus sends `enter` and then `motion` in the same frame (unless locked): Xwayland applies a frame's relative motion to its sprite and only an absolute motion corrects it.
- Pointer moves not caused by the device (surface moved under the pointer, compositor warps, lock hints) go through `SeatMut::warp`: v11 pointers get `wl_pointer.warp` in a frame of its own, older ones a motion.
- Focus changes outside input delivery (`warp` onto another surface, `set_pointer_grab` with `GrabFocus::Clear`, a grab ending and refocusing) send their own `wl_pointer.frame`; clients such as rikka-seat hold enter/leave until a frame arrives.
- `zwp_text_input_v3` v2: input-panel requests become `SeatEvent::InputPanel`, actions are validated but never performed, and language, action and preedit-hint events are never sent because input-method v2 cannot carry them. Content hints are masked to what the text input's version allows before reaching the IM.
- A host can be the input method itself: `SeatMut::serve_text_input(true)` enters text inputs without an IM client, `SeatEvent::TextInput { state }` reports enable/disable, surrounding text and content type, and `SeatMut::compose(&Composition)` delivers preedit, commit and delete-surrounding (rikka-nested uses it).
- Cursor, tool cursor, warp requests, cursor hints, caret, input popups, input panel, virtual pointer input and grab changes reach the host as `SeatEvent`s.
- Xwayland keyboard grabs pin keyboard focus to their surface; X11 focus side effects stay in the host (`KeyboardFocus::X11`).
- `SeatMut::withhold_keyboard()` sends `leave` and clears `entered` but keeps the focus, so clients hear no keys or modifiers and cannot activate; focus changes while withheld are only recorded, and `restore_keyboard()` enters whatever is focused then, with the keys still held. Every withhold needs a restore. The window switcher uses it.
- Idle timers run on the reactor; the host calls `activity()` on input and `set_idle_inhibited` after judging inhibitor visibility.
- Input method, virtual keyboard and virtual pointer managers are privileged-only; the Xwayland grab manager is visible only to the Xwayland client. Seats are never removed.
- Tests run a real `rikka-wayland-client` against a threaded display; `tests/common` holds a self-contained xkb keymap so no XKB data is needed.
