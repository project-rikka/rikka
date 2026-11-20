---
title: Animate holds (hands and pins gripping cloth)
description: animate-hold's keyed Hold spans, the Easy palm-ring grasp, hold timeline rows, finger pinch and the cloth Grab path.
about: apps/general/animate/crates/animate-hold
tags: [animate, cloth]
---

- `animate-hold` (GPU-free model + resolution): `Hold { holder: Holder::{Hand(Side), Pin{node, at}}, target: HoldTarget { figure: Option<name>, at: rest point, reach }, from, until, ease }`. Strength is whole from `from` to `until - ease`, easing in over the `ease` frames before `from` and out over those before `until`.
- `Holds` addresses holds by stable `HoldId` (handed out by `lay`, kept across `retime`; files and merges get fresh ids), keeps one hand from holding twice (laying a grab ends the hand's earlier hold), `retime` clamps between the same hand's neighbours, `lanes()` gives a lane per hand and packs pins into lanes. Saved in the holder's `.rmotion` (`Take.holds`, `HoldFile`, field `holds`).
- Other figures are named by `HoldTarget.figure` and resolved to the first other figure with that name; `None` is the holder itself. Pins ride the holder's own bone.
- `Pinches::new(body, model)` puts each hand's pinch (`Body::pinch`, thumb/index tips under `HandPreset::Pinch`) in the hand node's frame; `active_grabs(time, performers)` turns active holds into `cloth::Grab`s per owner (≤ `MAX_GRABS`), `grip_points` gives the holding hands' points (only holds whose cloth resolves).
- App: `TakePlayback.holds` carries the take's holds to the stage; `perform` blends the fingers toward `Pinch` by `Holds::closing` (`Body::write_fingers`). `animate-stage` `physics/` and the `LiquidScene` grabs feed the grabs to the cloth. A held grab never lets go: the cloth follows the hand however far it goes.
- Easy mode (pose first, then attach): each hand with a pinch shows a ring around its palm marker while `Stage::graspable` (cloth simulated). Dragging the ring (`animate-viewport` `easy/grasp.rs` `Grasping`) draws a band to the pointer; `ClothSnapshot::spot_at` ray-casts the live proxy triangles (nearest particle fallback) so any point of any figure's cloth can be chosen; the wheel widens the grasp; release over cloth lays the hold, with the pending pose registered in the same undo step if there is one (`Gesture::ParticlePick`, else `Gesture::Hold`). Clicking a holding hand's ring or a holding point marker lets go; G lets go of the subject's holds. There are no Alt gestures and Easy mode cannot create pins.
- `ClothSnapshot` is a particle readback taken only while a ring drag needs it (`easy::EasyDragState::feels`).
- Timeline: the Easy (`Listing::Parts`) sheet carries hold rows (`Row::Hold`, `HoldSpan { id, span, ease }`); hold and pour bars drag through the one `animate_track::Span::dragged(Edge, ..)` rule; a chosen span is deleted with Delete. The Hand viewport card lists that hand's current/chosen holds (Width, Ease, Let go); pins get their own Pins card.
- Edits go through `Animate::change_holds` (`update::<Take>`, an `Edit::Replaced` snapshot sharing the untouched tracks). `Holds::lanes()` gives `LaneHolds { lane: HoldLane, holds }`.
