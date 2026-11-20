---
title: Animate morphs (expressions) model and editing
description: Faces/Panel catalog, per-morph Morphs tracks on every Take, MorphPlayback with auto-blink, pending morphs, the panel.
about: apps/general/animate
tags: [animate, morphs]
---

- `animate_body::Faces::from_model(model)` is a figure's morph catalog, held by `Figure::faces` and `CastMember::faces`: PMX morphs in model order with their `Panel` (`Eyebrow`, `Eye`, `Mouth`, `Other`; PMX panel 0 is `None`, hidden), VRM expressions as channels, or plain mesh morph targets (presets found by `Expression::recognize`).
- `Expression::recognize(name)` knows each preset's names across VRM 1.0/0.x, MMD, VRoid `Fcl_*`, ARKit blinks and VRChat visemes; `Faces::adopt(name)` maps a foreign morph name onto this figure (exact or loose name, else the recognized expression's preset shares, else nothing).
- `Faces::write`/`write_amounts` take the `Instance`: VRM faces set `Avatar` expression weights (so overrides, material and UV binds apply), others write pose weights; `perform` clears avatar expressions each frame, and an expression-type VRM lookAt gets its gaze from the eye muscles (`Avatar::set_gaze`).
- `Expression` (moods, blinks, vowels) are presets over morphs: `Faces::express` writes a preset's morphs, `Faces::strength` reads it back from its first morph, `Faces::has` says whether the model has it. `Faces::resolve` matches a name exactly, then width- and case-normalized.
- `MorphWeights` is morph weights by name; `Faces::write` turns it into clamped per-node weights. Hot paths use `MorphAmounts` instead: weights by morph index (`Faces::index`, `amounts`, `strength_in`, `express_in`, `write_amounts`).
- Every `Take` (human or not) holds `morphs: animate_motion::Morphs` (= `Tracks<MorphKey>`, one track per morph name, looked up by `&str`); `MorphKey` carries weight and a `VmdCurve` for the segment arriving at it. `Take::morph_weight(name, frame)`/`morph_weights(frame)` sample them.
- `Take::from_vmd(target, vmd)` imports VMD morph frames losslessly as linear keys under `Faces::adopt`ed names; unmatched names are kept.
- `animate_motion::MorphPlayback` (in `TakePlayback`) resolves a take's morph tracks to indices once; `write(seconds, pending, instance)` samples them, overlays the pending weights and adds the seeded auto-blink of `HumanMotion::settings` unless blink or happy already close the eyes. `perform` calls it after the baked pose with the actor's pending morphs.
- `.rmotion` figures save morph keys at top level (`morphs`: name, frame, weight, curve bytes); a `detail.morphs` field is refused.
- Figure sheets get morph rows from `Sheet::of_take(listing, faces, take)`: one closed group per panel (`GroupKind::Morphs`), keyed hidden morphs under Other, names the model lacks under Unmatched. Lanes are `Lane::Morph(name)`, keys `Key::Morph(MorphKey)`, curves edit through `Channel::Value`.
- App logic lives in `animate/morphing.rs`: `express(change)` turns edits into `Pending::morphs` (only weights that differ from the take), `set_morphs`, `revert_morphs`, `zero_morphs`, `toggle_morph_keys` (keys the morphs at their shown weights, or deletes their keys at the playhead when all are keyed there and none is pending), `morph_keys` for Register, and `curves.morph` for new keys.
- `animate-inspector` `inspector/morphs/` is the Motion tab section for any figure, folded by its header (`morphs-open` preference): panel tabs, a search over all panels (`Faces::search`), and rows grouped by `animate_body::group_sides` (`SidedMorph`) (左/右 prefix or suffix, `_L`/`.R`-style or camel `Left`/`Right` variants under their whole morph or shared name, expanded per group by `Animate::toggle_sides`).
- Each row is a `MorphBar` (`bar.rs`): name and value over a fill; horizontal drag slides, a click chooses the lane (or opens a group with no whole morph), a double click swaps in a text field (`type_morphs`/`apply_typed_morphs`/`drop_typed_morphs`); the key diamond is filled for a key here, tonal for keys elsewhere and faint otherwise.
- The Easy mouth marker drag sends `Gesture::Express(Aa, weight)`; Escape restores the starting weight.
