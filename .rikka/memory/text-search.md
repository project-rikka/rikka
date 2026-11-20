---
title: Text search across the desktop
description: rikka_ui::search is the one search model; find_bar draws it over editors, documents and terminals.
about: crates/ui/rikka-ui/src/search
tags: [search, ui]
---

- `Query` compiles a pattern under `Options` (case, whole word, regex) into a regex. `Hits` orders matches, and `Search` is the shared handle the bar and the subject both hold. `Marking` gives everyone the same two colours.
- Only `rikka-ui` depends on `regex`.
- Literals are escaped, whole word adds `\b`, and case is inlined as `(?i)`. `Query::expression` gives other engines the same pattern, as the terminal does with alacritty DFAs.
- `Search` holds the bar state, the tally (`3/12`), the pattern and replacement documents, and one pending `Step` the subject takes when drawn.
- `search::stepped(current, count, Direction)` is the one wrap-around rule.
- `find_bar(theme, &search).on_action(..)` applies each `Action` via `Search::act`. A closed bar draws nothing, so views include it unconditionally.
- `find_mark(key)` plus `FindBar::anchored_to(key)` float the bar over a surface without resizing it.
- `text_editor(..).search(&s)` handles counting, marks, stepping and replace-all in one undo.
- Counting runs on a `search::Scan` thread over a rope clone (`request`, `result`, `is_pending`, `take_navigation_target`, `reset`). It gives up early on a newer request and repaints the requesting node when a result lands, so nothing polls per frame. Marks cover the drawn lines ±8 KiB.
- `Search::reporting` and `is_looking` expose progress.
- `rich_text/markdown(..).found(..)` match against whole region text, so a match split across styles counts once. `.reveals(index)` on the holding collection brings the match near.
- Keys: the bar's fields own Escape, Enter, Shift+Enter and F3. The window owns `Ctrl+F`, `Ctrl+H`, and `Ctrl+Shift+F` in terminals.
