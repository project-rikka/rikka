+++
name = "Computer"
deferred = true

[limits]
idle_timeout_ms = 3000
window_timeout_ms = 10000
width = 1280
height = 800
longest_seconds = 30
turns = 3
most_turns = 30
most_repeats = 100

[params]
actions = "Actions run in order, each an object with `action` and that action's fields. Put a sequence whose outcome you can predict in one call."
action = "One of: `start`, `close`, `screenshot`, `zoom`, `windows`, `cursor_position`, `mouse_move`, `left_click`, `right_click`, `middle_click`, `double_click`, `triple_click`, `left_mouse_down`, `left_mouse_up`, `left_click_drag`, `scroll`, `key`, `hold_key`, `type`, `wait`, `launch`."
coordinate = "Target `[x, y]` in screenshot pixels. Required for `mouse_move` and `left_click_drag`; optional for clicks, `left_mouse_down`, `left_mouse_up` and `scroll`."
start_coordinate = "Optional `[x, y]` start of `left_click_drag`; otherwise the current pointer position."
region = "`[x0, y0, x1, y1]` for `zoom`: the top-left and bottom-right corners in screenshot pixels."
text = "Keys for `key` or `hold_key`; characters for `type`; command for `launch`; modifiers such as `ctrl+shift` for clicks, `left_click_drag` or `scroll`."
repeat = "Presses for `key`: 1 by default, at most {{most_repeats}}."
scroll_direction = "One of `up`, `down`, `left`, or `right`."
scroll_amount = "Wheel turns: {{turns}} by default, at most {{most_turns}}."
duration = "Seconds for `wait` or `hold_key`; at most {{longest_seconds}}."
width = "Screen width for `start`; {{width}} by default."
height = "Screen height for `start`; {{height}} by default."
desktop = "Whether `start` opens the desktop. True by default; false starts a bare screen."
+++

Controls an isolated Rikka machine with its own screen, sharing this filesystem but not the person's session. After `start`, every program `{{bash_tool}}` starts opens there, never on the person's desktop; `close` ends the machine and its programs.

Each call runs `actions` in order, letting the screen settle (at most {{idle_timeout_ms}} ms) after each change, and ends with a screenshot unless the screen is unchanged since your last one. `screenshot` and `zoom` always return an image; `windows` and `cursor_position` answer in text. The first failing action stops the rest, and the reply names both.

- Coordinates are full-screenshot pixels from the top-left, also after `zoom`, which shows `region` at full resolution for small text and dense controls.
- Batch predictable steps, such as clicking a field, typing and pressing `Return`; look again before acting on what changed.
- Clicks hit `coordinate`, or the pointer, holding the modifiers in `text`; `mouse_move` only hovers.
- Keyboard shortcuts reach menus, lists and dropdowns more reliably than the pointer.
- `launch` runs the command in `text` and waits up to {{window_timeout_ms}} ms for its window.
