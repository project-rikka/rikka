---
title: Shared network client
description: rikka-net owns async and blocking HTTP, transfers, SSE, WebSockets, retries, TLS and cancellation.
about: crates/foundation/rikka-net
tags: [network, http]
---

- Built on reqwest, Tokio, rustls and tokio-tungstenite. `Client` builders, shared-client free functions, and a `blocking` mirror. Only HTTP(S)/WS(S) URLs are accepted (sealed conversion).
- A lazy two-worker `rikka-net` runtime; the caller's runtime is used where safe.
- `Transfer<T>` runs in the background and exposes progress and result to event loops without awaiting.
- Downloads resume, verify checksums, keep sidecars, and claim the destination atomically at the end.
- `SocketHandle` owns its task and sends from any thread. The inbound backlog is bounded by count and bytes (`Backpressure` closes). The outbound queue is bounded.
- `EventDecoder` parses SSE without EventSource reconnection; `last_event_id()` and `reconnection_time()` give a caller what it needs to resume a stream itself (rikka-mcp does).
- Error statuses stay responses until `error_for_status` (`StatusError` keeps the body). Cancellation covers body reads, so a cancel never looks like success.
- Retries: idempotent transient failures only, jittered backoff, bounded `Retry-After`. Uploads never retry.
- TLS trusts only the system roots. One process-wide rustls config (`tls::client_config`, built with an explicit provider on the first WebSocket connect) serves sockets, because programs may link both ring and aws-lc and then rustls has no process default; HTTP uses reqwest's own rustls setup.
- `DiskCache::open(folder, max_age)` is the keyed disk cache (sha256-named private files; a `rikka-net-cache` thread removes entries unused for `max_age` while `open` returns): `read(key)` (marks the entry used), `write(key, bytes)`, `get_or_fetch(url)` reads or fetches blocking, `path_for(key, extension)` names a file for downloads. Weather, music artwork and chat pictures/media use it; the freedesktop thumbnail caches and music's sized audio cache keep their own layouts.
