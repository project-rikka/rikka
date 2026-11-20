---
title: Line-number edits and their mistake gates (rikka-file-edit)
description: Files::edit pipeline over Op line ranges, the echoed-line and broken-syntax refusals, and what each gate compares.
about: crates/foundation/rikka-file-edit
tags: [ai, editing]
---

- `Files::edit(&EditRequest { path, ops })` → `EditOutcome { path, summary, excerpt, change }` resolves each `Op` (`op.rs` `Op::resolve` → `ResolvedOp`), refuses overlaps (`sort_disjoint`), builds one `Edited` (`edited.rs` `Edited::apply`: `before`, `after`, one `Splice { old, new }` per op in file order), runs both gates (`check_echoes`, `check_syntax`), then writes atomically against the bytes it read (a file changed meanwhile is `Error::ConcurrentModification`). `Files::with_history(&history)` gives `RecordedFiles`, whose `write`/`edit` first `History::record` the before-image for the checkpoint set by `History::set_checkpoint`; `History::rewind` returns a `RewindReport`. `Files::write` returns `WriteOutcome { path, relative_path, lines, created }`.
- Every refusal writes nothing and is an `Error` whose text starts `error: <kind> <path>: nothing was written.` followed by the lines involved, so a model can retry without listing the file again.
- `seam.rs` (`echoed_line`): new text whose first line repeats the kept line above its range, or whose last line repeats the kept line below, is refused. A neighbour touched by another splice, a range whose own first or last line already equals it, and lines without letters or digits (blanks, closing brackets) are exempt.
- `structure.rs` (`broken_syntax`): files with a loadable `rikka_grammars::Grammar` are parsed after the edit, and before it only when the result has faults. Old faults are carried through `Edited::shifted_line`. A clean file refuses any new fault; a file that was already broken refuses only when its fault count grows.
- A parse refusal starts with the bracket imbalance `Grammar::depths` counts from the tree's anonymous tokens, because tree-sitter often reports a dropped bracket as a wide unparsed span.
- Shader-like files without a grammar (`brackets.rs` `BRACKET_CHECKED`: glsl, comp, vert, frag, wgsl, hlsl, cu…) only have `{}`, `()` and `[]` lexed into `rikka_grammars::Depths`, skipping comments and `"`/`'` literals. A balanced file may not become unbalanced.
- Other files (plain text, unknown extensions) get only the echo gate.
- The grammar shipped for Rust misses Rust 2024's `safe fn` in `unsafe extern` blocks; otherwise the workspace's own files parse without faults.
