---
title: Animation primitives and motion tokens
description: rikka-animation's Spring/Curve/Channel and the rikka-ui token table every widget moves by.
about: crates/ui/rikka-animation
tags: [animation, ui]
---

- `rikka-animation` has no dependencies and owns no clock or thread.
- Callers advance a `Channel` (value plus velocity) with `advance(curve, target, elapsed)`. Compositor, UI and tests all evaluate motion the same way.
- A spring keeps its displacement from the target exactly (private `gap`), so it rests however small the steps are, even far from zero; an outside write to `value` is noticed and re-measured.
- `Presence` is a reversible 0 (hidden) to 1 (shown) progress: `advance(shown, curve, elapsed)` turns back from where it is, a spring keeping its momentum and a tween lasting only the share of its duration left to travel (CSS's reversing shortening). Every show/hide animation goes through it: `PaintCtx::progress` (`Motions::present`) and `.transition`s.
- `Curve` is `Spring(Spring)` or `Tween { easing, duration: DurationToken }`. `Spring::new(damping_ratio, stiffness)` follows Material, and `overshoots()` reports underdamping.
- `MotionScheme::{Expressive, Standard}.spring(Property::{Spatial, Effects}, Speed::{Fast, Default, Slow})` gives Material's spring slots.
- `Motion` (`Emphasized` default, `Standard`, `Custom`, `CustomSpring`) is what `View::animate` and `with_animation` take. `Motion::enter/exit` are emphasized decel/accel tweens.
- `crates/ui/rikka-ui/src/motion/tokens.rs` is the one table. Widgets name tokens, never numbers.
- `LIFT` (ζ 1, settles in ~200 ms) is the hover lift of an icon that grows in place.
- Spatial tokens are springs: `MORPH, TOGGLE, VALUE` (fast); `POP` (ζ 0.9, readable within ~100 ms); `RESHAPE` (default); `EMERGE` (softer, bouncier: a surface swelling out of another); `GLIDE`, `INDICATOR`, `SCROLL`, `DRAG` (critically damped, never overshoot). List rows move by `GLIDE`.
- Selection and hover highlights (`motion::indicator::Indicator`) keep their exact size while they travel; only their position animates. `Indicator::vanish` drops one at once, without fading.
- Effect tokens are springs too: `HOVER, PRESS` (fast), `COLOR, FILL` (default), `RIPPLE_FADE` (slow).
- A ripple wave (`rikka-widgets` `controls/ripple.rs`) stays opaque until `RIPPLE` has spread it to the far edge, then fades by `RIPPLE_FADE`, even after a quick click.
- Arrivals and departures are tweens: `RIPPLE, ARRIVAL, TURN_IN, TURN_OUT, POP_OUT, CARET, UNVEIL, SLIDE_IN, SLIDE_OUT, COLLAPSE`. `slide()` enters by `SLIDE_IN` and leaves by `SLIDE_OUT`. rikka-widgets' crate-private `lasting(curve)` turns a tween token into its `Duration`.
- `dismissible(child)` (`rikka-widgets` `blocks/dismissible/`) is a list slot that never clips: `.arriving(true)` opens its height from 0 by `ARRIVAL` while the child springs in at once from past the surface's trailing edge by `SLIDE_HOME` (ζ 0.9, under a pixel of overshoot), drawn beneath the slots below it; `.dismissed(true)` slides it out by `SLIDE_OUT`, then closes the height by `COLLAPSE`. Owners drop the item `Dismissible::DEPARTURE` after dismissing it and should set `dismissed` in the same handler that answers `on_dismiss`. It intercepts a trailing pointer drag or touchpad stroke once it travels past `DRAG_SLOP` (a vertical start is refused for the whole gesture), follows it, and on release flings away (40% of its width or 500 px/s, then `on_dismiss`) or springs home by `SLIDE_HOME` carrying the release velocity. Glass children slide, never fade or pop: blur regions have no opacity, so a fade leaves a bare blur.
- `rich_text(..).unveiling()` fades in letters appended since its last paint, by `UNVEIL` from each piece's arrival instant, through per-glyph span inks; `streamed_markdown` turns it on for every run and lets each new block rise in.
- Fading pieces are published to a thread-local registry; any `rich_text` first painted with text that starts with a fading text takes the pieces over, so a stream swapped for static markdown keeps fading.
- A `.transition` fires on every creation after a window's first show, including lazy re-realization, and a removed child with one lingers as a ghost; wrap it in a plain container to arrive without leaving.
- Each transitioning instance holds a `motion::Passage` (a `Presence` plus the current leg's start pose and progress). Leaving mid-arrival or reappearing mid-exit starts a new leg from the pose actually drawn, so asymmetric transitions (`shared_axis_*`, `fade_scale`) never jump.
- An element that reappears while its ghost still leaves revives that ghost (`reconcile.rs` `revivable`/`revive`): same key, or for unkeyed elements the same identity, branch and child index. Key a conditional panel by what it shows when different content should arrive fresh instead.
- `Ui::rejoin()` undoes `Ui::leave()`: `is_exiting()` turns false again, the next view revives the leaving content and it turns back.
- A ghost is out of flow and leaves from where it was last drawn, even when removing it moves its parent (an end-aligned or `resizing_upward` pile shrinks from the top); its offset is taken on its first walk (`Exit::shift`, added to its drawn bounds, not its layout rect) and kept, so it then rides with its parent. A ghost culled when removed takes no offset.
- Only a child list makes ghosts: an element returned as a `component` root is replaced in place, so it never leaves and a changed key does not arrive again. A popup a component shows goes inside a container the component always returns.
- `Ui::arrive()` makes the first view's transitions play too. Arrival clocks start on the frame after a `Ui`'s first drawn one, so a slow first frame never eats the entrance.
- `TURN_IN_DELAY < TURN_OUT`, so an arriving page waits for the leaving one.
- Enter/exit transitions (`fade, fade_scale, pop, slide, shared_axis_*, container_transform, wipe`) are `rikka_ui::motion` values applied with `.transition(..)`.
- Popups share one motion: `pop()` fades in and scales 0.92→1 by `POP`, out by `POP_OUT`. On an anchored element it scales about its anchor's attachment point (target edge centre, or the pointer for beside/offset marks) clamped into the element, so it grows out of what it hangs from; elsewhere about its centre (`arrange.rs` pivots, `Pose::about_anchor`).
- Panels painted inside another widget pop the same way: `PaintCtx::popped(shown, pivot, draw)` (opacity plus `PaintCtx::zoomed(factor, pivot, draw)`, a scoped uniform scale of everything drawn, clips included); `PaintCtx::pop_subtree(shown, pivot)` does it for a node's children. `Dropped::pivot()` (from `drop_panel`/`point_panel`) is the pivot for a dropped panel.
- Each instance keeps its running tracks in `Motions`: channel tracks (one inline) and `Presence` tracks, both `motion::track::Track`s searched linearly. `jump`/`fling` on a key drop its presence track, and the next `progress` on that key starts from the placed value and velocity, so "show at once" and flung gestures hand over to `progress`. Runtime-owned keys are `const` hashes (`motion_key("rikka:opacity")`); widget keys go through `hash_of`.
- The compositor has its own `animation/flight.rs` for window motion.
