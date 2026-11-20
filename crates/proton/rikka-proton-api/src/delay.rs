use std::time::Duration;

use rand::Rng as _;

#[must_use]
pub fn doubling(first: Duration, failures: u32, cap: Duration) -> Duration {
    2u32.checked_pow(failures)
        .and_then(|factor| first.checked_mul(factor))
        .map_or(cap, |wait| wait.min(cap))
}

#[must_use]
pub fn jittered(wait: Duration, fraction: f64) -> Duration {
    wait.mul_f64(1.0 - fraction + 2.0 * fraction * random_unit())
}

#[must_use]
pub fn stretched(wait: Duration, fraction: f64) -> Duration {
    wait.mul_f64(1.0 + fraction * random_unit())
}

#[must_use]
pub fn random_unit() -> f64 {
    rand::rng().random::<f64>()
}
