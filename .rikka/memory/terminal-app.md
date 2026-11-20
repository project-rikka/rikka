---
title: Terminal application
description: The Terminal window owns split panes of tabs and window chords over the shared rikka-terminal grid.
about: apps/system/terminal
tags: [application, terminal]
---

- The app owns windows, panes, tabs and chords. PTY, drawing and clipboard belong to `rikka-terminal`. It forwards only `Report::Ask`. Output repaints the grid through the session's watchers and views nothing; answers and unchanged titles `skip_view`.
- `panes.rs` is a tree: `Panes::Leaf` holds a `Group` of tabs, `Panes::Split` a `Split` (axis, ratio). Messages carry the group id.
- `Panes::top_row` hands top-edge strips to the title bar as `CaptionBar::columns`. Lower panes draw their own strip. Strips and panes are components keyed by group id; `Terminal.shape`, `Group.shape`, `Split.ratio` and `Tab.title` are the tracked keys, so a title re-views only its strip. The split skeleton and the caption's shares live in the window's root view, so a split drag re-views the title bar and skeleton (about half the window), never the panes.
- Dropping on a pane's middle joins its tabs, and on an edge splits the pane. A pane emptied by the drag takes the tab back.
- A closed last tab gives its room to the neighbour. The last pane closes the window.
- `keys::command` (tabs, `Ctrl+Shift+5`/`Ctrl+Shift+'` split, `Alt+arrow` focus, font size) is the grid's `on_key`. Only the active pane's grid is `active`.
- Each tab holds a `Session` with a rewireable reporter, so a dragged tab keeps its PTY.
- A second process start opens a window at that start's working directory.
- Font-size chords write `rikka.terminal` after a quiet period. The family defaults to JetBrains Mono via fontconfig.
- Search is per tab on `Ctrl+Shift+F`, so programs keep `Ctrl+F`, and floats over the grid.
- `opening.rs` reads a launch's `--dir=PATH` and `-e`/`--` command (also from a handed-off `LaunchRequest`); the window's first tab runs that command instead of the shell, matching the desktop entry's `X-TerminalArgExec=-e` and `X-TerminalArgDir=--dir=`.
