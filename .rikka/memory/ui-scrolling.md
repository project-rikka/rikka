---
title: rikka-ui scrolling (Viewing, latching, anchoring, Place)
description: Scroller follow rules, gesture latching, dominant axis, spring impulses, scroll anchoring, Place, scroll repaints.
about: crates/ui/rikka-ui
tags: [ui, scrolling]
---

- Every scroller owns one `Viewing` (scroll, grip, bar, follow and park rules). Only the reader moves a follower: any wheel, touchpad, thumb or key move toward the start detaches it, and only a move toward the end that reaches it rejoins it; content shrinking or growing never does. A follower with nothing to scroll is always attached; on rejoin a lagging offset glides (`homing`).
- Scroll latching (`runtime/latch.rs`): the node that handles the first event of a scroll gesture receives the rest of it without hit-testing, so a nested scroller passing under a still pointer never takes over. A gesture ends on a touchpad `Ended` or `Began`, 500 ms without events, a modifier change, or the pointer straying past `DRAG_SLOP`.
- Single-axis scrollers take only the dominant axis of a delta (`bar::travel`; a horizontal list also takes a plain wheel's vertical), and the text editor scrolls only when its dominant axis can move.
- Wheel and touchpad nudges impel the `motion::SCROLL` spring (`PaintCtx::impel`, `Curve::launch_speed`), so a critically damped scroll eases out from the first frame and notches in flight add up; aims and snaps drop the pending impulse.
- Scroll anchoring (`runtime/scroll_anchor.rs`): before each solve every dirty scroller records an anchor node, the first one starting within view below the top edge (none at offset 0), or after a press inside the scroller the deepest node under the pointer. `Arranged::anchor()` gives a `ScrollAnchor { was, is, touched }` (`moved()`), so the scroller shifts to keep it still; followers `settle` instead of snapping to the foot after a touch.
- `Place` is where the reader stands: the app owns it and passes `.place(&Place)` to scrollers, collections and editors to restore the spot without animation. `.shows(subject)` opens a new subject at the top.
- Reveals (`PaintCtx::reveal(rect, Reveal)`, `container.reveal(Reveal)`, `Widget::reveal`) bubble to the nearest scroller that can move. `Reveal::Nearest` scrolls minimally and is swallowed in the frame a scroller restores a `Place` or opens a new subject; `Reveal::Start` is navigation: it aligns the target to the top and is never swallowed.
- Scrolling moves the scroller's island group only; nothing is repainted, re-emitted or revisited except nodes entering view and tracking nodes.
