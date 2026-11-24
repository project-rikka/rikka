use std::hash::Hash;

use rikka_ui::{Axis, Element, Transition, View, fade, scale};
use rikka_widgets::AnimatedPosition;

const GROWN_FROM: f32 = 0.4;

const SETTLED_FROM: f32 = 0.85;

#[must_use]
pub(crate) fn grows() -> Transition {
    scale(GROWN_FROM) + fade()
}

#[must_use]
pub(crate) fn settles() -> Transition {
    scale(SETTLED_FROM) + fade()
}

#[must_use]
pub(crate) fn glide(
    axis: Axis,
    child: impl View,
    transition: Transition,
    key: impl Hash,
) -> Element {
    AnimatedPosition::default()
        .along(axis)
        .holding(child)
        .transition(transition)
        .keyed(key)
}
