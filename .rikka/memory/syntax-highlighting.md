---
title: Syntax highlighting
description: rikka-syntax parses with tree-sitter over rikka-ui's text model; rikka-theme owns each token colour.
about: crates/ui/rikka-syntax
tags: [ui, syntax]
---

- Grammar identity and loading live in the foundation crate `rikka-grammars` (`Grammar::named`, `for_path`, `language()`, `syntax_errors(text)`), shared with `rikka-file-edit`'s syntax gate. rikka-syntax's own table keys highlighting facts by grammar name.
- Grammars are not linked into programs. Each is a shared object `libtree-sitter-<name>.so`, dlopened by `rikka-grammars` on the language's first use and kept for the process.
- Search order (`rikka_shipped::Shipped`): `RIKKA_GRAMMAR_PATH` (`bazel run //xtask -- app`), `<program>/../lib/rikka/grammars` (installed), then the `RIKKA_GRAMMARS` folder baked in at compile time (tests, `bazel run`). A missing grammar or a failing query logs a warning and leaves text uncoloured; queries compile once per language.
- `nix/packages/grammars.nix` builds each `.so` from the tree-sitter crate source at the one version `Cargo.lock` locks, listing each grammar's crate and source folder; the dev shell exposes the result as `RIKKA_GRAMMARS`. The queries come from the same pinned crate sources: each grammar crate has a `queries` filegroup annotation in `crates.MODULE.bazel` (`@crates//:tree-sitter-<x>-queries`), and `//crates/ui/rikka-syntax-queries` embeds the listed `.scm` files into the generated `src/generated/queries.rs` (table `_QUERIES` in rikka-syntax's `BUILD.bazel`). No target depends on a grammar crate's Rust library, so no parser is compiled by Bazel; the specs stay only as version pins for `grammars.nix`.
- API: `Language::named` (fence word or alias), `Language::from_path`, `Language::spans` (one-shot snippet), `Parsed::spans(content, within, colors)` (per-editor incremental).
- Editing facts per language: `Language::comment()` (`Comment::Line`/`Block`), `quotes()` (auto-closed quote characters), `openers()` (extra block openers such as Python's `:`), and `is_word(language, ch)` (letters, digits, `_`, plus the grammar's joiners like CSS `-`).
- Completion: `Parsed::lexicon(content, at)` walks the named leaves of the tree within 256 KiB of the caret and sorts each word into a `Kind` from node kind, parent kind and field name; keywords are the grammar's anonymous word nodes (`Rules::keywords`). Without a tree, `Lexicon::from_text` gathers plain words. `Lexicon::suggest(typed, most)` ranks by `rikka-fuzzy` score (smart case, first letter on a word or hump start), then distance to the caret. `Parsed::token_at` tells comments and strings apart so typing there stays quiet.
- Adding a language means an entry in `rikka-grammars`' `table.rs` (name, C symbol, aliases, extensions, file names), one in rikka-syntax's `grammar.rs` (queries and comment, quote, opener and joiner facts), its tree-sitter crate as a `crate.spec` plus the `queries` annotation in `third_party/rust/crates.MODULE.bazel`, its query files in `_QUERIES`, and an entry in `grammars.nix`.
- `slang` (also `hlsl`) comes from `tree-sitter-slang`, which ships no queries: rikka-syntax uses C's highlight query plus its own `src/slang.scm` (the valid parts of C++'s query and Slang keywords). The crate is only a test dependency (its spec pins the version `grammars.nix` builds); a test checks the shipped `.so` against it.
- A child language lists parent queries first, because the last match wins.
- Capture names resolve to `rikka_theme::Token` by dropping dotted suffixes until one matches.
- `Parsed` edits its tree from `Content::changes_since` and parses within a per-frame budget, resuming next frame. Edits drop only the injection layers they touch.
- Only the visible byte range is queried, injections included.
- Snippets share a thread-local cache. Colours never enter it: spans carry `Token`s, resolved against `Theme::syntax()` at paint, so a theme change costs no reparse.
