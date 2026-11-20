---
title: Seam capability IPC
description: rikka-seam's typed capabilities, exactly-once replies, per-subscriber event queues, forwarding and Unix sockets.
about: crates/session/rikka-seam
tags: [ipc, session]
---

- Dependency-light IPC for the compositor, shell, tools and settings. Manifests declare versioned methods and events, and the registry rejects anything undeclared.
- Payloads are serde values. Error codes sit in stable semantic bands. Remote failures stay `Remote { code, message }`.
- Each `Call` is answered exactly once. `Call::defer` hands off responsibility, and a dropped responder answers `NoReply`.
- Services run on the host thread through a `Dispatcher`, never on socket threads.
- Services are host-typed: `Service<H>::call(&mut self, host: &mut H, method, call)` on a `Seam<H>` (`H = ()` by default). A seam kept outside its host uses `seam.dispatch(&mut host, call)`; a seam owned by its host uses `Seam::dispatch_within(host, call, |host| &mut host.seam)`, which lends the service out for the call. `register` returns the `ServiceContext`, so a host keeps the publisher itself; services generic over every host (`SettingsService`) implement `impl<H> Service<H>`.
- Transport: length-prefixed JSON over Unix sockets (`SocketListener`, `SocketClient`). The runtime dir is 0700, sockets 0600, with no per-app authorization.
- Sockets live in `$XDG_RUNTIME_DIR/rikka/<WAYLAND_DISPLAY>/`: the compositor's `seam.sock` first, then every other `*.sock` (the shell's `shell.sock`). `net::session_sockets` lists them.
- `RIKKA_SEAM_SOCKET` selects the session. `SocketClient::for_capability(cap)` finds the serving program, and `manifests` asks a socket what it serves.
- Clients subscribe while connecting. Each subscriber has its own pre-filtered queue.
- `EventInfo::new(..).latest()` marks state events, where newest overwrites. Other events are all kept.
- A `Delivery` is an `Event` or `Lagged` (count and capabilities). Consumers answer `Lagged` with a fresh read. `EventInfo::latest()` sets `DeliveryMode::Latest`.
- `Seam::forwarding_to` plus `relay` let the shell serve locally and forward the rest. Local wins.
- Blocking waits (`EventQueue::blocking_next`, `next_timeout`, `Reply::blocking_wait`) use std parking, so they are safe in Tokio. Connection loss closes buses with no reconnect, because compositor loss is session loss; `SocketClient::closed().await` resolves on loss and `is_connected()` reads the same state.
