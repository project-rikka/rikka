---
title: Direct scan-out (rikka-compositor-planes)
description: rikka-compositor-planes' pure assign() and presentation() pick scan-out, tearing flips, VRR and pacing per frame.
about: crates/compositor/rikka-compositor-planes
tags: [compositor, kms]
---

- Clients reach the display only through the primary plane; everything else is composited.
- `rikka-compositor-planes` is pure (geometry + `rikka_compositor_types`; it does not depend on the KMS crate): `assign(&Display, Option<&Surface<S>>, refused: Fn(Format) -> bool)` returns `Assignment::{Primary(Layout { surface, format, flip }), Composited(Option<Blocked>)}`; tests in `tests/assign.rs` and `tests/presentation.rs` are one table each.
- `Display::new(mode, output transform, primary formats).with_tearing(Tearing)`, `Display::whole()` is the output rect; `Surface` carries format, buffer, source, transform, physical placement, `opaque`, `covered` (something is drawn over it) and its own `TearingHint`.
- `Layout::flip` is `Flip::Async` only when the display allows tearing (`rikka.windows/allow-tearing` and the card's atomic async page-flip cap) and the scanned-out surface hints `Async`; composited frames always flip on vblank.
- `presentation(VrrPolicy, fullscreen content, flip) -> Presentation { vrr, pacing }` is the content-type policy: `Automatic` turns VRR on while a fullscreen window shows `none`, `game` or `video` content (not `photo`); pacing is `Immediate` while tearing, `LowLatency` for games, `Steady` for video, else `Balanced`. The fullscreen content is the first non-`none` content type in the fullscreen window's surface tree, root first (`windows::fullscreen_surface`, none while an overview shows).
- Vocabulary shared with the surface and KMS crates lives in `rikka-compositor-types`: `ContentType`, `TearingHint`, `Flip`, `Pacing`.
- The caller hands the topmost candidate covering `Display::whole()` (`Composition::covering`); the KMS screen drops it first unless its colours need no conversion on that screen (`color::scans_out_unchanged`). It goes on the primary plane when its format is listed and not refused, it is opaque and uncovered, its buffer equals the mode, its source is the whole buffer, its placement is the display and its transform the output's. Otherwise `Blocked` names it and the `Reason`.
- Candidates are unclipped, full-opacity, shown, unmoved surfaces; none exist while post effects do.
- `Promotion::update` debounces each assignment: a surface reaches the primary plane only after being the choice for 100 ms with at least two new buffers; it leaves at once. `Reason::Pending` marks the wait, `Promotion::reset` forgets the candidate; dmabuf advice follows the raw assignment.
- KMS side (`backend/kms/`): `screen.rs` plans each frame, applies the presentation after it (`FramePacer::set_pacing`, `Device::set_vrr`; the display's `rikka.displays/<id>-adaptive-sync` choice is its `VrrPolicy`), hands `layout.flip` to `Present::flip`, `scanout.rs` imports client framebuffers, pins buffers until their flip retires and remembers refused or unimportable formats until the output is relit; `advice.rs` sends scan-out dmabuf feedback to a Format-blocked surface.
