---
title: Rust workspace conventions
description: Flat facades, semantic errors, deferred builders, no comments or stubs, and naming of threads and runtimes.
tags: [conventions, rust]
---

- Implementation modules are private; `lib.rs` exposes a flat `pub use` facade. Daemon crates and backend `rikka-input` modules keep their own namespaces (`Error`, `Action`, `Snapshot` repeat).
- Builders are `#[must_use]`, chainable and take `impl Into<…>`. Fallible options wait for the terminal call, and the first error wins.
- Errors: `src/error.rs`, semantic `thiserror` variants, `pub type Result<T, E = Error>`. Backend errors are translated, never leaked.
- Bad user catalogs, themes and settings are reported (`LoadReport`) and skipped. Developer mistakes are errors.
- No comments in Rust, Bazel files or Nix. No doctests and no Rust usage examples; the only examples are the SDK's in `sdk/examples/`. No `todo!`, `unimplemented!`, `TODO`, `FIXME`.
- Names use each domain's standard vocabulary (Wayland, Vulkan, OpenType, D-Bus, PipeWire…) and the Rust API Guidelines: `new`/`from_*`/`with_*` or `From` impls, never preposition constructors (`of`, `over`, `within`); no prose or metaphor names; locals name what they hold.
- Test names are short behaviour statements without articles (`queue_stops_at_end`). Log, error and `expect` messages are lowercase, terse and technical (`"failed to save page: {error}"`), with tracing fields where useful.
- Synchronization is event-driven (fds, channels, protocol callbacks); a fixed sleep exists only for genuine domain timing, as a named constant. `String`s are built through `fmt::Write` with `?`.
- Locks are `rikka_sync::{Mutex, RwLock, Condvar}` (std underneath, poison recovered inside: `lock()`/`read()`/`write()`/`wait*()` return guards, `try_lock()` an `Option`). Never `parking_lot`, `PoisonError::into_inner` tails or private `fn lock` helpers. Production code never panics on poison; `std::sync` locks remain only in tests and where a caller deliberately skips work on a poisoned lock (`let Ok(..) = x.lock() else`).
- Worker threads are named `rikka-*`. I/O Tokio runtimes use two workers.
- `unsafe_op_in_unsafe_fn` is denied. Unsafe code lives in the renderer, video FFI, audio's SPSC queue, rikka-simd (every `core::arch` intrinsic), rikka-text-catalog's font mmap, rikka-grammars' grammar loading, rikka-heap's allocator setting, rikka-ui (node-handle `Arc` reuse, video frame submission), rikka-seat (keymap fd), rikka-app (launch environment scrub, raw presentation and drag-ghost handles), rikka-keyring-vault's mlocked key page and the CPU physics engine (`crates/physics/rikka-physics-{math,core,geometry,collision,rigid}`: `Shared` views, row-pointer transposes, executor helpers).
- SIMD goes through rikka-simd only; in this toolchain even SSE2 intrinsics need `unsafe` outside a `#[target_feature]` function.
- Public `Debug` never prints secrets.
- LF line endings everywhere. TTF files go through Git LFS.
- Size rule: the linker already drops unused code of linked crates, but anything reachable through a runtime branch (a format enum, a registry, a language table) lands in every program. Heavy optional pieces are typed choices (image encodings) or loaded at run time (grammars).
