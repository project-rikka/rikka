---
title: Movable shell overlay windows
description: widgets::floating draws a shell overlay window dragged by any empty part that remembers where it was left.
about: system/rikka-shell/crates/shell-kit/src/widgets
tags: [shell, ui]
---

- `floating(spot, panel, on_dismiss)` = dismissing scrim + `sheet` (keeps Tab, takes keys) + entrance and exit + movement. Options: `.on_key .width .down(portion)`; it plays its exit once `rikka_ui::leaving()` turns true.
- Translate, markup and trim use it. A new one needs a `Spot` in its feature's `Shared`.
- `Spot` is `Rc<Cell<Option<Point>>>` and outlives the surface. It is unrelated to `rikka_ui::Place`.
- `grip` catches unhandled presses anywhere in the sheet (3 px threshold), writes the `Spot`, and calls `request_view`.
- A moved window stands at `Styled::position`, clamped into the room at paint. Otherwise it is centred.
- `entrance()` is shared by `floating` and `popover`. `shell_kit::EXIT_DURATION` is the exit duration passed to `.animated`.
