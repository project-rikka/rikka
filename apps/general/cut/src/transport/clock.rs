use std::time::Instant;

use cut_render::Playing;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Clock {
    at: f64,
    since: Option<Instant>,
    speed: f64,
    requested: Option<(f64, Instant)>,
}

impl Clock {
    pub(crate) fn frame(&self, now: Instant, per_second: f64) -> f64 {
        let Some(since) = self.since else {
            return self.at;
        };
        let elapsed = now.saturating_duration_since(since).as_secs_f64();
        (self.at + elapsed * per_second * self.speed).max(0.0)
    }

    pub(crate) fn speed(&self) -> Option<f64> {
        match (self.since, self.requested) {
            (Some(_), _) => Some(self.speed),
            (None, Some((speed, _))) => Some(speed),
            (None, None) => None,
        }
    }

    pub(crate) fn running(&self) -> Option<Playing> {
        self.since.map(|since| Playing {
            from: self.at,
            since,
            speed: self.speed,
        })
    }

    pub(crate) fn is_running(&self) -> bool {
        self.since.is_some()
    }

    pub(crate) fn asked_at(&self) -> Option<Instant> {
        self.requested.map(|(_, at)| at)
    }

    pub(crate) fn ask(&mut self, speed: f64, now: Instant, per_second: f64) {
        match self.since {
            Some(_) => {
                self.at = self.frame(now, per_second);
                self.since = Some(now);
                self.speed = speed;
            }
            None => self.requested = Some((speed, now)),
        }
    }

    pub(crate) fn run(&mut self, now: Instant) {
        if let Some((speed, _)) = self.requested.take() {
            self.speed = speed;
            self.since = Some(now);
        }
    }

    pub(crate) fn stop(&mut self, now: Instant, per_second: f64) {
        self.at = self.frame(now, per_second).floor();
        self.since = None;
        self.requested = None;
    }

    pub(crate) fn seek(&mut self, frame: f64, now: Instant) {
        self.at = frame.max(0.0);
        if self.since.is_some() {
            self.since = Some(now);
        }
    }
}
