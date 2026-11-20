use rikka_animation::{Curve, DurationToken, Easing, Spring};

const fn tween(easing: Easing, duration: DurationToken) -> Curve {
    Curve::Tween { easing, duration }
}

pub const HOVER: Curve = Curve::Spring(Spring::FAST_EFFECTS);

pub const PRESS: Curve = Curve::Spring(Spring::FAST_EFFECTS);

pub const COLOR: Curve = Curve::Spring(Spring::DEFAULT_EFFECTS);

pub const FILL: Curve = Curve::Spring(Spring::DEFAULT_EFFECTS);

pub const RIPPLE: Curve = tween(Easing::Standard, DurationToken::Medium2);

pub const RIPPLE_FADE: Curve = Curve::Spring(Spring::SLOW_EFFECTS);

pub const ARRIVAL: Curve = tween(Easing::EmphasizedDecel, DurationToken::Short4);

pub const TURN_IN: Curve = tween(Easing::EmphasizedDecel, DurationToken::Short4);

pub const TURN_IN_DELAY: DurationToken = DurationToken::Short2;

pub const TURN_OUT: Curve = tween(Easing::EmphasizedAccel, DurationToken::Short3);

pub const SLIDE_IN: Curve = tween(Easing::EmphasizedDecel, DurationToken::Medium4);

pub const SLIDE_HOME: Curve = Curve::Spring(Spring::new(0.9, 500.0));

pub const SLIDE_OUT: Curve = tween(Easing::EmphasizedAccel, DurationToken::Short4);

pub const COLLAPSE: Curve = tween(Easing::Standard, DurationToken::Short4);

pub const POP_OUT: Curve = tween(Easing::StandardAccel, DurationToken::Short2);

pub const CARET: Curve = tween(Easing::StandardDecel, DurationToken::Short2);

pub const UNVEIL: Curve = tween(Easing::StandardDecel, DurationToken::Medium3);

pub const POP: Curve = Curve::Spring(Spring::new(0.9, 1000.0));

pub const MORPH: Curve = Curve::Spring(Spring::FAST_SPATIAL);

pub const TOGGLE: Curve = Curve::Spring(Spring::FAST_SPATIAL);

pub const VALUE: Curve = Curve::Spring(Spring::FAST_SPATIAL);

pub const INDICATOR: Curve = Curve::Spring(Spring::new(1.0, 520.0));

pub const LIFT: Curve = Curve::Spring(Spring::new(1.0, 900.0));

pub const RESHAPE: Curve = Curve::Spring(Spring::DEFAULT_SPATIAL);

pub const GLIDE: Curve = Curve::Spring(Spring::new(1.0, 400.0));

pub const EMERGE: Curve = Curve::Spring(Spring::new(0.6, 220.0));

pub const SCROLL: Curve = Curve::Spring(Spring::new(1.0, 440.0));

pub const DRAG: Curve = Curve::Spring(Spring::new(1.0, 2000.0));
