+++
name = "Computer"
deferred = true

[limits]
idle_timeout_ms = 3000
window_timeout_ms = 10000
longest_seconds = 30
turns = 3
most_turns = 30
most_repeats = 100

[params]
actions = "Actions run in order, each an object with `action` and that action's fields. Put a sequence whose outcome you can predict in one call."
action = "One of: `screenshot`, `zoom`, `windows`, `cursor_position`, `mouse_move`, `left_click`, `right_click`, `middle_click`, `double_click`, `triple_click`, `left_mouse_down`, `left_mouse_up`, `left_click_drag`, `scroll`, `key`, `hold_key`, `type`, `wait`, `launch`."
coordinate = "Target `[x, y]` in screenshot pixels. Required for `mouse_move` and `left_click_drag`; optional for clicks, `left_mouse_down`, `left_mouse_up` and `scroll`."
start_coordinate = "Optional `[x, y]` start of `left_click_drag`; otherwise the current pointer position."
region = "`[x0, y0, x1, y1]` for `zoom`: the top-left and bottom-right corners in screenshot pixels."
text = "Keys for `key` or `hold_key`; characters for `type`; command for `launch`; modifiers such as `ctrl+shift` for clicks, `left_click_drag` or `scroll`."
repeat = "Presses for `key`: 1 by default, at most {{most_repeats}}."
scroll_direction = "One of `up`, `down`, `left`, or `right`."
scroll_amount = "Wheel turns: {{turns}} by default, at most {{most_turns}}."
duration = "Seconds for `wait` or `hold_key`; at most {{longest_seconds}}."
+++

Works the person's own desktop through a pointer and keyboard of your own, drawn as a coloured ring. Theirs stay where they are, so never ask them to move anything; what you click takes your typing without taking their focus.

Each call runs its `actions` in order and lets the screen settle after every action that changes it. A call ending on such an action returns one screenshot of the display your pointer is on, or says the screen is unchanged since your last screenshot. `screenshot` and `zoom` always return an image. If an action fails, the rest are not run and the reply says which failed and which were skipped.

- Coordinates are pixels of the full screenshot from top-left `[0, 0]`, also after a `zoom`.
- Batch steps such as clicking a field, typing and pressing `Return`; look again before acting on what changed.
- `zoom` shows `region` at full resolution, for small text and dense controls. `cursor_position` and `windows` answer in text.
- `mouse_move` hovers. Clicks use `coordinate` when given, otherwise the pointer, and hold modifiers from `text`. `left_mouse_down` and `left_mouse_up` press and release separately; `left_click_drag` drags from `start_coordinate` or the pointer to `coordinate`.
- `scroll` turns the wheel by `scroll_amount` in `scroll_direction`, optionally at `coordinate` and with `text` modifiers.
- `type` enters `text`. `key` presses a shortcut such as `ctrl+s`, `alt+Tab`, `Return`, `Escape`, `F5`, or `super`, `repeat` times; `hold_key` holds it for `duration`. Shortcuts reach menus, lists and dropdowns more reliably than the pointer.
- `launch` starts the command in `text` in this session and waits up to {{window_timeout_ms}} ms for its window. `wait` pauses for `duration`.
- The screen counts as settled once it stops changing, at most {{idle_timeout_ms}} ms after an action.
- An X11 window under Xwayland answers to one pointer only, so clicking one moves the person's pointer with yours; say so rather than fighting it.
