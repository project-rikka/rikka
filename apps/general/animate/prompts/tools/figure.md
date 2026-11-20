+++
name = "Figure"

[params]
figure = "The figure's name as `{{scene_tool}}` gives it."
frame = "The frame to read; the playhead's when omitted."
+++

Reads a humanoid figure's pose at a frame as JSON, in `{{pose_tool}}`'s terms: `hips`, non-zero `joints` in degrees, `expressions`, non-zero keyed `morphs` with the frames of each morph's keys in `morphKeys`, `gaze` with each eye's keyed [across, up], `pins`, stage `points` of hips, head, palms and ankles in metres, `height`, whether joint `limits` apply, and per-part `keys` with tags.

- The pose is interpolated from keys, without physics, blinking or secondary motion.
- Aim `reach`, gaze and camera targets from `points`.
