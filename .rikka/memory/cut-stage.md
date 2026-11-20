---
title: Cut preview stage (projected manipulation, motion path, gizmo, camera)
description: apps/general/cut/src/stage: items seen through shot projections, motion path keys, anchor, 3D rings, camera overview gestures.
about: apps/general/cut/src/stage
tags: [video, ui]
---

- The stage reads `LivePreview::project_or` (`Stage::shown_project`) and builds items from `Project::shots_restyled(frame, view, overrides)`, exactly as cut-render's compositor does; paint passes the same project, overrides (live, typing draft) and viewpoint to the render `Request`, so pictures and overlays always agree.
- `item_projection.rs` `ItemProjection` maps an item's flat px (where `Visual` geometry, grips and `figure::Manipulation` work) to stage project px: shot projection after the inverse of the keyed `Placement::affine`. Hits, grips, outlines, typing and region outlines all go through it; `Viewing` then maps project px to widget px.
- `manipulating.rs`: body drags run in screen space (shift and snapping on projected bounds), then each member moves by its own unmapped delta; grip drags run `Manipulation` in one shared flat space (only when all selected items have matching `ItemProjection`s). Results pass through `Placement::slid`, so 3D items move within their tilted plane.
- `route.rs` draws the selected clip's keyed position (`Track<Spatial>::path` through `Shot.path`), per-frame dots, keys and tangent handles, built by `Route::of` from `Stage::animated_preview` (for animated clips only: the shown project plus that clip's `LivePreview` appearance applied as a `Redraw`; paint computes it once for route and ghosts), so it follows every preview. Drags preview `Edit::Key` via `Cut::preview_edit` and commit with `commit_edits`. The playhead key is skipped unless Alt is held (it sits on the anchor).
- `ghosts.rs`: for a single selected tweening clip, dashed outlines show the item at the tween's start and end frames (the one under the playhead is skipped), from the same previewed project.
- `anchor.rs`: with `Tool::Anchor` (Y, AE's Pan Behind) a drag on an item or the selected item's cross moves the anchor and slides position so the item stays put; with Select the cross is only drawn. `gimbal.rs`/`spinning.rs`: X/Y/Z rings from `Placement::ring` planes (via `Project::shot_as`) and a depth knob; face-on rings follow the pointer angle, flat ones slide along their tangent.
- `framing.rs`: a single selected camera at the playhead (Select tool) switches the view to `Viewpoint::Overview` with a fitted scale (frozen during a gesture); body pan, corner zoom, knob roll, Alt orbit, wheel dolly (`Cut::dolly` folds a burst into one undo step).
- Every stage gesture previews through `Cut::preview_edit` (`Gesture::redrawn` gives item/camera gestures their `Edit::Redraw`) and commits one edit on release (`Cut::redraw` or `commit_edits`).
