use std::time::Instant;

const MIN_SPEED: f64 = 0.01;
const MAX_SPEED: f64 = 64.0;

#[derive(Debug)]
pub(crate) struct PlaybackClock {
    origin: Option<Instant>,
    base_us: i64,
    speed: f64,
}

impl PlaybackClock {
    pub(crate) fn new() -> Self {
        Self {
            origin: None,
            base_us: 0,
            speed: 1.0,
        }
    }

    pub(crate) fn is_running(&self) -> bool {
        self.origin.is_some()
    }

    pub(crate) fn now_us(&self) -> i64 {
        self.at(Instant::now())
    }

    fn at(&self, now: Instant) -> i64 {
        match self.origin {
            Some(origin) => {
                let elapsed = now.saturating_duration_since(origin).as_micros() as f64;
                self.base_us + (elapsed * self.speed) as i64
            }
            None => self.base_us,
        }
    }

    pub(crate) fn wall_us(&self, media_us: i64) -> i64 {
        (media_us as f64 / self.speed) as i64
    }

    pub(crate) fn anchor(&mut self, media_us: i64) {
        self.base_us = media_us;
        self.origin = Some(Instant::now());
    }

    pub(crate) fn set(&mut self, media_us: i64) {
        self.base_us = media_us;
        if self.origin.is_some() {
            self.origin = Some(Instant::now());
        }
    }

    pub(crate) fn set_speed(&mut self, speed: f64) {
        self.rebase(Instant::now());
        self.speed = speed.clamp(MIN_SPEED, MAX_SPEED);
    }

    pub(crate) fn pause(&mut self) {
        self.base_us = self.now_us();
        self.origin = None;
    }

    pub(crate) fn resume(&mut self) {
        if self.origin.is_none() {
            self.origin = Some(Instant::now());
        }
    }

    fn rebase(&mut self, now: Instant) {
        self.base_us = self.at(now);
        if self.origin.is_some() {
            self.origin = Some(now);
        }
    }
}
