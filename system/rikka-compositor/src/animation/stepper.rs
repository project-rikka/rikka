use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Stepper {
    stepped: Option<Instant>,
}

impl Stepper {
    pub(crate) fn step(&mut self, now: Instant) -> Duration {
        let elapsed = self.stepped.map_or(Duration::ZERO, |stepped| {
            now.saturating_duration_since(stepped)
        });
        self.stepped = Some(self.stepped.map_or(now, |stepped| stepped.max(now)));
        elapsed
    }
}
