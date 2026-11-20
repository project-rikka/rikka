+++
name = "Pose"

[params]
figure = "The figure's name as `{{scene_tool}}` gives it."
keys = "Poses to key, in order."
frame = "The frame to key."
tag = "How the keyed parts approach this key: {{tags}}. Omitted, an existing key keeps its tag and a new key is `smooth`."
rest = "Parts to reset to the model's rest pose: any of {{parts}}, or `all`."
joints = "Degrees from the T-pose by joint, then by `bend`, `side` or `twist`. Only given angles change."
hips = "`position` [x, y, z] in metres, `yaw`, `pitch`, `roll` in degrees. Only given values change."
hands = "Hand shape by side (`left`, `right`): {{hands}}."
expressions = "Weights 0–1: {{expressions}}; each sets the model's morphs for it."
morphs = "The model's own morphs by name, weights 0–1."
gaze = "`mode` (`off`, `camera`, `point`), `target` [x, y, z] in metres for `point`, `weight` 0–1, `headFollow` 0–1, `eyes` by `both`, `left` or `right` as [across, up] each −1–1."
reach = "A point in metres for the palm of `leftHand` or `rightHand`, or the ankle of `leftFoot` or `rightFoot`; the body follows by IK within the joint limits."
pins = "Pin (true) or release (false) `leftArm`, `rightArm`, `leftLeg` or `rightLeg`; a pinned hand or foot holds its place on stage."
+++

Keys poses on a humanoid figure. Each key starts from the pose at its frame, including keys laid earlier in the call; applies `rest`, `joints`, `hips`, `hands`, `gaze`, `reach`, then `pins`, and keys exactly the parts that changed. `expressions` and `morphs` key each morph they set on its own track, so other morphs keep their timing. The answer gives, per key, the parts and morphs keyed, values clamped to limits, and where head, hands and feet landed.

- Angles are from the T-pose: upright, arms straight out with palms down, legs straight down. The model's rest pose is not zero; read it with `{{figure_tool}}`.
- Positive angles, in the figure's own terms:
  - spine, chest, upperChest, neck, head: `bend` forward, `side` lean to its left, `twist` turn to its left. Jaw `bend` opens.
  - Shoulder: `bend` shrug, `side` forward. Upper arm: `bend` raise (negative lowers toward the hips), `side` swing forward, `twist` roll the palm backward. Lower arm: `bend` the elbow, `twist` as the upper arm. Hand: `bend` lift the back, `side` toward the thumb.
  - Fingers: `bend` curl; `side` spreads away from the middle finger. Thumb `bend` folds across the palm.
  - Upper leg: `bend` lift forward (negative back), `side` spread out, `twist` toes out. Lower leg: `bend` the knee, `twist` toes out. Foot: `bend` lift the toes, `side` turn out. Toes: `bend` lift.
- Limits in degrees, clamped; 1.5× with the figure's limits off:
{{joints}}
- Hips `yaw` turns the figure to its left (90° faces +X), `pitch` leans forward, `roll` leans to its right. Lower the hips to crouch or sit; feet never sink below the floor.
- Gaze `eyes` turn the eyes within the head: `across` toward the figure's left, `up` upward, 1 as far as an eye turns (about 35° across, 25° up, 30° down); `left` or `right` overrides `both`. Mode `off` keeps the eyes where `eyes` put them, and `eyes` without a `mode` switch to `off`; `camera` and `point` turn them toward their target by `weight`.
- An arm or leg is keyed whole, shoulder to hand or hip to toes.
