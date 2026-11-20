---
title: Compositor output, capture, blur and DRM lease protocols
description: rikka-compositor-output's policy-free wl_output/xdg_output, ext image capture, background effect and DRM leases.
about: crates/compositor/rikka-compositor-output
tags: [compositor, wayland]
---

- Four built-in modules, each with a host trait on the compositor state: `OutputProtocols`/`OutputHost`, `CaptureProtocols`/`CaptureHost`, `BackgroundEffectProtocols`/`BackgroundEffectHost`, `DrmLeaseProtocols`/`DrmLeaseHost`. The crate decides no policy.
- `Outputs` owns every head from connection on and allocates its `OutputId` (`rikka_compositor_types`), the one id the compositor's backends, `Desktop`, `Surfaces` and the input route share. `connect(name)` knows a head without publishing it; `publish(&display, id, OutputInfo) -> Publication::{Published, Changed, Unchanged}` creates the wl_output v4 global or sends only changed events then `done`; `withdraw` removes the global (the head keeps its id) and `disconnect` forgets the head. The name never changes. `layouts()` hands out a `LayoutView` of every published head's `OutputLayout` (geometry, scale, transform), the one source `Desktop` reads. Bound wl_outputs are kept in a `rikka_compositor_server::ByClient`.
- `resources(output, client)` (bound objects grouped per client) and `output_of(&WlOutput)` (only a resource bound to the current publication resolves) let the compositor drive surface enter/leave and presentation. `OutputHost::output_bound` is where it sends enters for a newly bound wl_output.
- `Outputs` owns the shared `rikka_compositor_types::Layouts` table: `publish` writes `OutputInfo::layout()`, `withdraw` clears it, and `Outputs::layouts()` hands out the `LayoutView` that `Desktop` reads.
- `Outputs::done(output)` sends a bare `wl_output.done` (the compositor uses it after an image description change).
- xdg_output v3 is always published. From v3 its `done` is replaced by wl_output's `done`.
- Capture globals are privileged only. `Captures<T>` is generic over the host's toplevel id; `CaptureHost::toplevel_source` resolves an `ext_foreign_toplevel_handle_v1`.
- The host gives initial `Constraints` from `capture_session_started` (None stops the session) and receives `CaptureRequest`s from `capture_requested`. It answers with `Captures::ready`/`fail`; late answers return false.
- `Constraints { size, formats: Rc<CaptureFormats> }`: the host builds `CaptureFormats` (shm fourccs, dmabuf device and per-fourcc modifiers) once and shares it; equality is size plus `Rc::ptr_eq` or equal content. `Captures::is_pending(frame)` says whether a frame awaits the host.
- The crate checks shm targets against the constraints through `Surfaces::buffer_content`. The host checks any other buffer kind. The first ready frame of a session always carries full damage.
- Changed constraints and stopped sessions fail an in-flight frame. The compositor stops sessions of a vanished output with `stop_source`. It moves cursor sessions with `place_cursor`, which sends enter, leave, position and hotspot as diffs.
- The blur region is a `rikka_compositor_surface::Cached` value, so it follows commits, sync subsurfaces and blockers. Read it with `blur_region(&surfaces, id)`. `blur_region_committed` fires when a commit applied one.
- `DrmLeases` holds one privileged global per device fd (non-master; `open_lease_device` makes one). `offer`/`withdraw` manage connectors. `grant_lease` returns a `LeaseGrant`, with the ioctl helpers `create_lease`/`revoke_lease`.
- A client destroying its lease calls `revoke_lease`. When the compositor calls `end`, it gets the lessee back and revokes the lease itself. Lease devices live as long as the compositor. Leased connectors are withdrawn and offered again when the lease ends.
- Tests start a threaded display with every module plus minimal wl_seat and foreign-toplevel stubs. They use rikka-wayland-client as the client.
