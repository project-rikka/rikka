+++
name = "SetWind"

[params]
resting = "Values for settings without keys, by setting name: {{wind_settings}}."
keys = "Keys to lay; a key already at that frame and setting is replaced."
frame = "The frame to key."
values = "Values keyed at the frame, by setting name."
curve = "Easing from the previous key: {{curves}}, or [x1, y1, x2, y2] in 0–127. Omitted, an existing key keeps its curve and a new key is linear."
clear = "Settings whose keys are all removed, returning them to their resting value."
+++

Changes the scene's wind as one undo step: clears settings, sets resting values, then lays keys. The whole call is checked first; an unknown setting or a value out of range is refused with its range and nothing changes.

- A setting with keys follows its keys; its resting value shows only once they are cleared.
- Calm is `speed` 0; a breeze 2–4, a strong wind 8–15. Key `speed` over a few frames for a gust the person asks for.
