---
title: Local file operations (rikka-file-operations)
description: rikka-file-operations: copy/move/trash/delete engine with progress, conflicts, links and staged writes over a Store.
about: crates/desktop/rikka-file-operations
tags: [desktop, files]
---

- The one owner of local file operations; Files (`files-vfs`) and ADE (`ade-files`) both run them. Nothing else copies trees, moves across devices or names duplicates.
- `run(Arc<S: Store>, Task<L>) -> Operation<I>` runs on a `rikka-file-operation` thread. `Task::{Copy, Move, Relocate, Trash, Delete, Restore}`; `spawn(TaskKind, work)` + `Reporter` run other jobs (Files' compress, extract, convert) under the same `Operation`.
- `Operation`: `snapshot`, `updates()` (coalescing stream ending after the outcome), `wait`/`wait_until`, `resolve(Resolution, remember)`, `cancel`; dropping an unfinished one cancels it. `resolve(answer, true)` right after `run` is a standing answer for every conflict.
- `Outcome { cancelled, failures, placed }`: a batch keeps going past failures (a source that fails to be measured is reported once and skipped); `placed` lists every placement as it happened (Keep-both names, merge children), which Files' undo replays.
- Rules: same-named folders merge; a clash asks `Conflict { source, existing, destination }` (`Replace`, `Keep` = `rikka_format::unique_name` beside, `Skip`, `Cancel`); a copy onto itself becomes a numbered duplicate, a move onto itself is skipped, into itself fails. Symlinks are recreated, never followed. A move renames when `Store::can_rename_across` (same device), else copies then deletes the source. Files land through `Store::write` sinks committed only when complete, so cancel or failure leaves nothing and Replace keeps the old file until then. `delete` never removes a non-empty folder; the engine walks.
- `Store` (associated `Location: Address`, `Item: Item`) is the seam: `Local` is the filesystem over `PathBuf` with `Entry` (lstat plus followed target, `Link { target, broken }`, `EntryKind`); `files_vfs::Vfs` is a `Store` over `files_core::Location` dispatching to scheme backends (`Backend: Store`). `Local` also offers `create_file`, no-replace `rename` (a case-only rename of the same inode is allowed), `erase` (trash record), `set_metadata`, `is_writable`.
- `Error<L>` is generic over the location (`map` converts); Files aliases it as `files_vfs::Error`, ADE wraps it as `ade_files::Error::Operation`.
- Tests: `tests/operations.rs` covers conflicts, cancel mid-copy (a gated store), cross-device moves (a store that refuses renames), links and drafts; `tests/trash.rs` round-trips the trash.
