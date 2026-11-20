use std::time::Duration;

use rikka_ui::Curve;

pub(crate) const fn lasting(curve: Curve) -> Duration {
    match curve {
        Curve::Tween { duration, .. } => duration.duration(),
        Curve::Spring(_) => panic!("a timed motion must be a tween"),
    }
}
