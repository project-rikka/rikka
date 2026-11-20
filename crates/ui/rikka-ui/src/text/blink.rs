use std::time::{Duration, Instant};

use rikka_animation::Easing;

use crate::paint::PaintCtx;

const PERIOD: Duration = Duration::from_millis(1060);

const STOPS: [(f32, f32); 5] = [
    (0.0, 1.0),
    (0.45, 1.0),
    (0.55, 0.0),
    (0.95, 0.0),
    (1.0, 1.0),
];

#[derive(Clone, Copy, Debug, Default)]
pub struct Blink {
    since: Option<Instant>,
}

impl Blink {
    pub fn restart(&mut self) {
        self.since = None;
    }

    pub fn alpha(&mut self, ctx: &mut PaintCtx<'_>) -> f32 {
        let now = ctx.now();
        let since = *self.since.get_or_insert(now);
        let elapsed = now.saturating_duration_since(since);
        match steady_remaining(elapsed) {
            Some(until) => ctx.wake_at(since + until),
            None => ctx.request_frame(),
        }
        alpha(elapsed)
    }
}

fn alpha(elapsed: Duration) -> f32 {
    let progress = (elapsed.as_secs_f64() / PERIOD.as_secs_f64()).fract() as f32;
    STOPS
        .windows(2)
        .find_map(|stops| {
            let &[(from, start), (to, end)] = stops else {
                return None;
            };
            (progress <= to).then(|| {
                let eased = Easing::Standard.transform((progress - from) / (to - from));
                start + (end - start) * eased
            })
        })
        .unwrap_or(1.0)
}

fn steady_remaining(elapsed: Duration) -> Option<Duration> {
    let periods = elapsed.as_secs_f64() / PERIOD.as_secs_f64();
    let progress = periods.fract() as f32;
    let (to, steady) = STOPS.windows(2).find_map(|stops| {
        let &[(_, start), (to, end)] = stops else {
            return None;
        };
        (progress < to).then_some((to, start == end))
    })?;
    let until = PERIOD.mul_f64(periods.floor() + f64::from(to));
    (steady && until > elapsed).then_some(until)
}
