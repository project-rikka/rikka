---
title: Compositor X11 window manager (rikka-compositor-xwm)
description: Sans-IO Xwayland WM on x11rb-protocol: continuation replies, per-turn flush, batched property reads, selections.
about: crates/compositor/rikka-compositor-xwm
tags: [compositor, xwayland]
---

- The library speaks X11 through `wire/` (`Wire`): x11rb-protocol's sans-IO `Connection` over a non-blocking socket, its own packet reader and write buffer with BIG-REQUESTS lengths. The `x11rb` crate is only the test client.
- Only `Xwm::start` blocks, for three pipelined round trips (setup; atoms and extension queries; versions, BIG-REQUESTS and the redirect check). After that nothing waits on a reply.
- Host contract: watch `Xwm::as_fd`, call `dispatch_into(&mut Vec<XwmEvent>)` when it is readable, and call `Xwm::flush()` once per reactor turn. Commands only buffer requests. The compositor flushes in `Compositor::each_turn` (`flush_x11`).
- Replies are continuations: `pending.rs` queues an `ExpectedReply` per sequence number, with `Resume` steps that run after a later reply (atom names, interned atoms). Replies are answered in stream order before each event.
- Windows live in `Record`s (`record.rs`): the public `XWindow`, cached titles, cached `XdndAware`/`XdndProxy`, an epoch against id reuse, and dirty/in-flight property reads.
- PropertyNotify only marks fields dirty; one batch of GetProperty goes out per dispatch. `PropertyChanged` fires only when a value changed, once per window and property per dispatch.
- MapRequest, MapNotify, ClientMessage, ConfigureRequest and FocusIn wait (`deferred`) until their window's property reads have landed.
- `AtomNames` caches `Rc<str>` names in `FxHashMap`s, seeded with the startup atoms, bounded at 4096 entries.
- `XWindowId` and `MimeList` (`Rc<[Box<str>]>`) are `rikka_compositor_types` types shared with rikka-compositor-desktop and rikka-compositor-selection; mime lists appear in `set_selection`, `drag_enter`, `SelectionOffered` and `X11DragEntered`. A Wayland offer's atom list (`TARGETS`, `TIMESTAMP`, then its targets) is resolved once per offer and reused for every TARGETS reply and `XdndTypeList`.
- X to Wayland selections stream: GetProperty with `delete=true` in chunks as the pipe drains, one chunk buffer, no copies. Wayland to X reads straight into a chunk-sized buffer. The chunk comes from the maximum request length, capped at 1 MiB.
- Transfer pipes stay registered edge-triggered for their whole life; the deadline timer is only re-armed when a deadline gets earlier.
- Tests (`tests/support/session.rs`): `Session::turn` polls the WM fd plus the peer fds a call passes (`wait_for_serving`, `run_until`), `wait_until(&app, …)` blocks on the X client socket and hands each batch of events to the predicate, and the fake Wayland compositor signals serials through a condvar (`wait_for_serial`).
- `_NET_FRAME_EXTENTS` is the host's: `set_frame_extents(window, FrameExtents)` writes it (also before map), and `_NET_REQUEST_FRAME_EXTENTS` becomes `XwmEvent::FrameExtentsRequest` for the host to answer.
