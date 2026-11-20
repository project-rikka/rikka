---
title: ADE guides, skills and memory
description: Standing RIKKA.md instructions, lazy project/person skills, and project-only memory notes with a derived index.
about: apps/system/ade/crates
tags: [ade, memory]
---

- `ade-scope` does lenient front-matter reading (`matter::read`, `FrontMatterError`), project/person layering (`Scope`), name and text helpers (`strings`), and `Diagnostic`s (skipped or loaded-with-warning) for guides, skills and memory; each catalogue exposes `diagnostics()` and renders its prompt block with `render()`.
- Instructions come from `~/.rikka/RIKKA.md` then `<project>/RIKKA.md` (project wins), capped at 32 KiB, placed before the machine block. No other filenames are read.
- Nested `RIKKA.md` files are found on first write under them (walking toward the root) and delivered once, shallowest first, through `Nearby`.
- Skills: `.rikka/skills/<name>/SKILL.md` in project and user scope; project overrides person. The catalogue (names and descriptions) is a standing block (`prompts/skills.md`), so changes arrive as revisions; the `Skill` tool takes a plain name and keeps a fixed declaration. The body loads on invoke, and adjacent files load on request.
- Skill parsing repairs unquoted colons and skips bad entries into diagnostics. `allowed-tools` is shown but not enforced.
- Memory: only `<project>/.rikka/memory/*.md` (title, ≤120-char description, optional `about`, tags).
- Only titles and descriptions are injected. `MEMORY.md` is rendered from notes, grouped by `about`, never hand-edited.
- Guides, skills and memory are reread before each send, never mid-turn. A thread keeps the system prompt it began with; later changes reach it through `prompts/revised.md` as a revision on the next turn.
- Front matter (`ade_scope::matter::read`) is read by ade-scope's own flat reader (`fields.rs`): scalars (plain, quoted, `|`/`>` blocks, wrapped lines), flow and block lists, one-level maps; anything deeper is ignored. The `---` fence split is `rikka_prompt::FrontMatter::split`. `ade_scope::listing` lists a folder's Markdown files or subfolders for the catalogs.
