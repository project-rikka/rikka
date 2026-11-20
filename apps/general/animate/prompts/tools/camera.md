+++
name = "Camera"

[params]
keys = "Camera keys to lay."
frame = "The frame to key."
target = "The point looked at, [x, y, z] in metres."
eye = "Where the camera stands, [x, y, z] in metres; give this or the whole orbit."
yaw = "Degrees around the target: 0 in front on +Z, 90 on +X (the figure's left, screen right), 180 behind."
elevation = "Degrees above the target; negative looks up from below."
distance = "Metres from the target."
roll = "Degrees, positive counterclockwise; 0 when omitted."
fov = "Vertical field of view, 1–179°; 30 when omitted."
curve = "Easing from the previous key: {{curves}}, or [x1, y1, x2, y2] in 0–127. Omitted, an existing key keeps its curve and a new key is linear."
+++

Lays camera keys, each replacing any camera key at its frame. Place a camera by `eye`, or by `yaw`, `elevation` and `distance` together.

- A cut is two keys one frame apart; a hold is one key, or equal keys around it.
- Frame figures from their `{{figure_tool}}` `points`: head, chest or hips.
