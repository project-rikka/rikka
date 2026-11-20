use std::time::Instant;

const LONGEST_STEP: f32 = 0.1;
const CALM_SPEED: f32 = 0.03;
const REST_SECONDS: f32 = 0.5;

#[derive(Debug, Default)]
pub(super) struct Stillness {
    last: Option<Instant>,
    calm: f32,
}

impl Stillness {
    pub(super) fn elapsed(&mut self, now: Instant) -> f32 {
        let elapsed = self.last.map_or(0.0, |last| {
            now.saturating_duration_since(last)
                .as_secs_f32()
                .min(LONGEST_STEP)
        });
        self.last = Some(now);
        elapsed
    }

    pub(super) fn wake(&mut self) {
        self.calm = 0.0;
    }

    pub(super) fn watch(&mut self, motion: f32, elapsed: f32) {
        self.calm = match motion < CALM_SPEED {
            true => self.calm + elapsed,
            false => 0.0,
        };
    }

    pub(super) fn is_settled(&self) -> bool {
        self.calm >= REST_SECONDS
    }
}
