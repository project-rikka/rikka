+++
name = "Render"

[limits]
most_frames = 9
widest = 1280

[params]
frames = "Up to {{most_frames}} frames; the playhead's when omitted."
camera = "`film` through the camera motion (the free view when there is none), `view` through the person's view, or a placement as in `{{camera_tool}}`: `target` with `eye`, or `yaw`, `elevation`, `distance`, plus `roll` and `fov`."
width = "Pixels along the longer side of each frame, 64–{{widest}}; 640 when omitted. Frames take the output aspect ratio from Scene's `output`."
+++

Draws frames exactly as the stage renders them, physics and look included, and shows them to you and the person as one contact sheet read left to right, top to bottom.

- Several frames of one action show the motion; one frame through several cameras compares shots.
