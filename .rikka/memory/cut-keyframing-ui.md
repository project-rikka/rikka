---
title: Cut timeline key lanes, graph editor and preset wedges
description: Cut timeline key lanes and value/speed graph (timeline/keys), animation-preset wedges, keyframe selection.
about: apps/general/cut/src/timeline/keys
tags: [video, animation]
---

- `cut/keyframing.rs` holds `Keyframing` in `Cut`: lanes open (`U`, `Command::KeyLanes`, the clip-bar disclosure), graph on (`Shift+F3`, `Command::GraphEditor`), `Plot::{Value, Speed}`, the picked `KeyRef`s (timeline frames), plotted properties, the right-click pin point, whether the key menu is open, and copied `KeyPiece`.
- Interpolating and deleting picked keys go through the panel's shared `cut/animating.rs` (`ease_keys`, `remove_keys`). The key menu's edit-curve entry opens the panel's easing picker (`Popup::Easing(Picking::keys(..))`) anchored to the same pin, which `keys::pin` keeps on screen while the menu or that picker is open.
- Property names come from `wording::property(property, clip)` over `wording::{placement_field, art_field, camera_field}`, the same labels the panel rows use (`cut.rows.*`, `cut.panel.*`, `cut.camera.*`); `[cut.keys]` holds only timeline and interpolation wording.
- Lanes open only under `Cut::opened_clip()` (the single `selected_animated()` clip while open); `picked_keys()` is empty otherwise and drops refs that no longer exist; `Cut::choose` calls `forget_keys`.
- `Command::run` routes Cut/Copy/Delete/Deselect to keys while keys are picked; Paste pastes keys at the playhead while copied keys are held; copying clips drops them, so the newest copy wins. `Command::Interpolate` (F9 / Shift+F9 / Ctrl+Shift+F9 and the key menu) applies an interpolation to the picked keys.
- `cut/previewing.rs`: `preview_edit`/`preview_edits(&[Edit])` show a gesture through `LivePreview::rework` (which ignores results equal to what is shown); `commit_edits(Vec<Edit>)` commits the first changing edit then `amend`s the rest (one undo step) and settles the preview. Every timeline drag previews per move and commits on release.
- `timeline/rows.rs` `Rows::opened(layer, height)` leaves an `Opening` under the opened layer; `at` and `across` account for it. `timeline/mod.rs` positions the `Keys` widget absolutely in that gap and `headers.rs` inserts `keys::header` (summary bar toggles + `Names`) after the layer's header.
- `keys/outline.rs` is the expansion geometry (summary row, one `LANE` per keyed property from `Clip::properties()`, or a graph at least `GRAPH` tall); `marks.rs` collects lane/summary marks; `glyph.rs` maps a `Timing` to AE halves of a `rikka_timeline::KeyGlyph` (diamond linear, hourglass eased, circle auto, square hold); `shift.rs` is the time drag over `rikka_timeline::KeyShift` (clip reach, snapping); presses pick by `rikka_timeline::press` and bands are `Band`s; `gesture.rs` the press/drag/release state machine.
- `keys/graph/`: `Plotting` fits each curve to the graph height on its own (frozen during a gesture); the value graph plots only scalar channels, the speed graph every channel; `points.rs` gives key and handle positions (handles only for picked keys); `handle_drag.rs` turns handle drags into `KeyChange::Handle` (Alt adds `Smooth::Manual`).
- `lanes/wedges.rs` draws In/Out animation presets as eased ramps on clip bars; dragging a ramp's inner edge (`Part::AnimationPreset`) previews and commits `Edit::AnimationPresets`.
