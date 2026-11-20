---
title: Shared PipeWire helpers (rikka-pipewire)
description: rikka-pipewire owns the PipeWire connection, loop worker, sync round trip, F32 format pod and SPA pod helpers.
about: crates/foundation/rikka-pipewire
tags: [pipewire, audio]
---

- Every PipeWire client in the workspace goes through this crate; none calls `pipewire::init`, `MainLoopRc::new` or `ContextRc::new` itself.
- `Connection::open()` / `open_fd(fd)` gives `main_loop()`, `core()`, `registry()`. Build it on the thread that runs it; it is not `Send`.
- `spawn(name, |stopping| setup -> Ok(run))` starts a `LoopWorker` (a `rikka_worker::Worker`): setup errors return synchronously and dropping the worker stops the loop.
- `connection.run_until_stopped(stopping)` runs the loop until the worker is dropped. `connection.run_with(commands, |c| ControlFlow)` does the same for a crate's own `pipewire::channel` of commands (`Break` quits).
- `connection.settle(timeout)` is one `core.sync` round trip, for one-shot registry listings; `false` means the server did not answer in time.
- `float_format(rate, channels)` is the F32LE `EnumFormat` pod; `None` leaves the rate or channel count to the graph.
- Pod helpers: `object`, `meta`, `Fields`, `RawFormat`/`Sample`, `OfferedRates`, `RATES`, `rates_within`, `number`/`numbers`, `default_id`, `default_rectangle`.
- Users: rikka-audio (engine, capture, app linking), rikka-hifi (shared output), rikka-screencast (frames), rikka-cast, rikka-sound, rikka-privacy.
