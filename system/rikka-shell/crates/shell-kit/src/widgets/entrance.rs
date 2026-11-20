use rikka_ui::{Curve, DurationToken, Motion, Property, Spring, Transition, fade, scale};

pub const EXIT_DURATION: DurationToken = DurationToken::Short4;

const ENTER_FROM: f32 = 0.94;

#[must_use]
pub(crate) fn entrance() -> Transition {
    (fade() + scale(ENTER_FROM))
        .enter(Curve::Spring(Spring::DEFAULT_SPATIAL))
        .exit(Motion::exit(EXIT_DURATION).curve(Property::Effects))
}
