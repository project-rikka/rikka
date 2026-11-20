---
title: Compositor shell roles (rikka-compositor-shell)
description: Policy-free xdg/layer/lock/foreign/Xwayland shell state machines as Modules; host decides through ShellHost events.
about: crates/compositor/rikka-compositor-shell
tags: [compositor, wayland]
---

- Install `XdgShell::new().icon_sizes(..)`, `LayerShell`, `SessionLock`, `ForeignToplevelList` and `XwaylandShell` as modules; the host owns `Shell::new(&display)` and implements `ShellHost: SurfaceHost` (`shell()`, `shell_event(ShellEvent)`, `admit_activation(&TokenRequest)`).
- `XdgShell` covers xdg_wm_base, decoration, activation, xdg_foreign v2, dialog, toplevel icon/tag/drag and system bell. Layer shell, session lock and the foreign toplevel list are privileged-only globals; the Xwayland shell global is offered only to `Client::is_xwayland()` clients.
- Every policy choice is the host's: it answers `InitialCommit` with `configure_toplevel/popup/layer/lock_surface` (each returns the serial), picks decoration modes inside `ToplevelConfigure`, confirms or finishes locks, and publishes foreign toplevels itself.
- Ids are slotmap keys (`ToplevelId`, `PopupId`, `LayerId`, `LockId`, `LockSurfaceId`, `ForeignToplevelId`) so a recreated role object on the same wl_surface never aliases an old one. `shell_role(&surfaces, surface)` maps a `SurfaceId` to its `ShellRole`.
- Records live in `Shell` slotmaps; the surface role payloads (`XdgRole` assigned at get_xdg_surface, `LayerRole`, `LockRole`, `XwaylandRole`) only point at them.
- Double-buffered role state is latched in `Role::pre_commit` into a per-record FIFO and applied in `Role::committed`, so fence-delayed commits keep their order. `lifecycle.rs` owns configure/ack/initial-commit/map/unmap rules for xdg and layer surfaces.
- A layer commit emits `Changed` (new state) before its `Mapped`/`Unmapped` transition, so the host decides mapping with the state that commit applied.
- A surface keeps at most 64 unacked configures (`configures.rs`). Older ones are dropped (`AckResult::Dropped`); acking one of those is still valid (it configures without new state), and acking a kept one forgets the rest.
- Foreign toplevels are published from `ForeignToplevel::Xdg(toplevel)` (names read from the shell's own record) or `ForeignToplevel::Named { title, app_id }` with borrowed strings; `update_published_toplevel` sends only changed names and nothing at all when none changed.
- Property changes arrive as payload-free events (`ToplevelEvent::Title`...); the host reads values from `shell.toplevel(id)`. Requests (move, resize, grab, reposition) carry their arguments.
- `Positioner::geometry(constraint)` is the pure, spec-literal flip/slide/resize solver; `placement()` is the unconstrained rect. Both are relative to the parent's window geometry.
- Ping timeouts run on the reactor through `Shell::ping(client, patience)`; tokens and export handles are `rikka_compositor_types::unguessable()` (128 bits of kernel randomness as hex).
- Tests use a threaded display plus `rikka-wayland-client`; `tests/common` serves dummy wl_output, wl_seat and wl_data_device_manager globals.
