---
title: Screen capture (rikka-screencast)
description: rikka-screencast is the ScreenCast portal client and PipeWire frame consumer apps use directly; sound is rikka-audio's.
about: crates/desktop/rikka-screencast
tags: [desktop, capture]
---

- Apps depend on it directly; it is the consumer side of the portal that `system/rikka-cast` serves.
- `ScreenCast::new().sources(Sources::ANY).cursor(true).frame_rate(fps).start()` blocks through the portal picker (run it in `spawn_blocking`) and returns a `Cast`; `Error::Cancelled` means the person closed the picker.
- Persistence is opt-in: `persist(true)` or `restore_token(token)` asks for `persist_mode` 2, and `Cast::restore_token()` hands the token back. `frame_rate` clamps to 1–240.
- `Cast::next_frame()` gives `NextFrame::Frame | Pending | Ended(why)` from a latest-frame-wins handoff without blocking, and the `Cast` is `AsFd`: its eventfd turns readable when a frame or the end arrives, so callers poll it beside their own fds; `recycle(frame)` returns pixel buffers to a small pool. Dropping the `Cast` stops PipeWire and closes the portal session.
- A `Frame { width, height, captured, picture }` holds `Picture::Pixels(Pixels { order, stride, bytes })` copied from SHM buffers (cropped by `SPA_META_VideoCrop`) or `Picture::Dmabuf(Dmabuf)`: a dequeued PipeWire buffer held until the frame drops (then requeued on the loop through a command channel, unless the buffer was removed meanwhile), with dup'd fds, `key() -> BufferKey` (stable per added buffer) and `with_spec(size, |DmabufSpec| …)`; corrupted buffers are skipped. `Cast::offer_dmabufs(&[DrmFormat])` renegotiates EnumFormat (dmabuf pods with MANDATORY|DONT_FIXATE modifiers first, the copied formats last; an empty list goes back to SHM); the producer fixates the modifier, and an unsettled format is ignored. A negotiated format keeps the offer's `Choice` wrappers (type `None`), so `capture/format.rs::read` unwraps them; failing that, every buffer is dropped.
- One `rikka-screencast` thread per cast runs a PipeWire main loop on the portal's remote fd; video capture threads are `rikka_pipewire::LoopWorker`s spawned through `rikka_pipewire::spawn`, which sets the stream up on the thread, reports setup errors synchronously and stops the loop through a PipeWire channel.
- It captures no sound: desktop and per-app sound come from `rikka_audio::AudioCapture` (see the audio-engine note).
- The crate carries no tests: its pure parts are private and the portal and PipeWire need a session.
