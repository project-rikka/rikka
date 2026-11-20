---
title: Rope document model (rikka-text-document)
description: rikka-text-document owns the in-house byte-indexed Rope and the editable Content under rikka-ui and rikka-syntax.
about: crates/text/rikka-text-document
tags: [text, editing]
---

- Internal crate over `rikka-text-layout` only (for `TextSource` and word boundaries). `rikka-ui::text` re-exports everything; `rikka-syntax` depends on it instead of rikka-ui, so the highlighter builds beside the UI framework.
- `Rope` is a persistent treap of UTF-8 chunks (at most 1 KiB, planted at 512 bytes) with byte, char and `\n` counts per subtree. `clone` is O(1); edits copy only the path they touch. Every offset is a byte offset and must sit on a char boundary, as with `String`.
- API: `len`, `char_count`, `line_count`, `line_of(byte)`, `line_start(line)`, `line_span(line)` (without its `\n`), `byte_to_char`/`char_to_byte`, `floor_char_boundary`/`ceil_char_boundary`, `chunk_at(byte) -> (&str, start)`, `chunks(range)`, `chars_from`/`chars_before`, `char_at`/`char_before`, `slice(range)`, `insert(at, &str)`, `remove(range)`, `Display`, `==` with `str` and `Rope`.
- A short insert or removal inside one chunk edits that chunk in place; anything else splits and merges. When chunks average under 64 bytes the rope replants itself, so typing never fragments it.
- Only `\n` is a line break. `Content` normalises every inserted text (`LineEnding::normalize`) in its one mutation path, so `\r` never reaches the rope; tree-sitter points count rows the same way.
- `Content::travel(Travel, extend, Option<&ParagraphLayout>, WordRule)` is the one caret movement (`erase(Erase, WordRule)` and `select_word_at(at, WordRule)` take the same rule; callers choose `Prose` or `Code`): with the caret line's layout Left/Right step visually (`ParagraphLayout::visual_step`) and cross to the neighbouring line at the paragraph's visual edge by its direction, Home/End go to the shown row's ends (smart Home toggles indentation on a line's first row), word arrows go forward or back by the paragraph direction; without a layout everything is logical by grapheme. The caret carries an `Affinity` (`affinity()`, `place_cursor`): insertion and selection ends face back (`Before`), placement and deletion forward. A selection stays the logical range between anchor and caret; Delete removes a grapheme, Backspace one character.
- `Content` is the caret, selection, undo history (512 steps of O(1) rope snapshots), preedit, ghost text and the `Splice` log (`changes_since`) that incremental parsing reads. `TextDocument` in rikka-ui is the shared, observed wrapper.
- `tests/rope.rs` checks random edits against `ropey`, which is a test-only dependency.
