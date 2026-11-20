use std::sync::Arc;
use std::time::Instant;

use rikka_model::mmd::FRAMES_PER_SECOND;
use rikka_sync::Mutex;

const ON_FRAME: f32 = 1e-3;

pub type SharedClock = Arc<Mutex<Clock>>;

pub fn with_clock<R>(clock: &SharedClock, use_clock: impl FnOnce(&mut Clock) -> R) -> R {
    use_clock(&mut clock.lock())
}

pub fn frame_of(time: f32) -> u32 {
    let frames = f64::from(time) * f64::from(FRAMES_PER_SECOND);
    let slack = f64::from(ON_FRAME) + frames.abs() * f64::from(f32::EPSILON);
    (frames + slack).floor().max(0.0) as u32
}

pub fn time_of(frame: u32) -> f32 {
    frame as f32 / FRAMES_PER_SECOND
}

#[derive(Clone, Copy, Debug)]
pub struct Clock {
    at: f32,
    since: Option<Instant>,
    length: f32,
    repeat: bool,
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            at: 0.0,
            since: None,
            length: 0.0,
            repeat: false,
        }
    }
}

impl Clock {
    pub fn time(&self, now: Instant) -> f32 {
        let Some(since) = self.since else {
            return self.at;
        };
        let running = self.at + now.saturating_duration_since(since).as_secs_f32();
        match running < self.length {
            true => running,
            false if self.repeat && self.length > 0.0 => running.rem_euclid(self.length),
            false => self.length,
        }
    }

    pub fn is_running(&self) -> bool {
        self.since.is_some()
    }

    pub fn has_ended(&self, now: Instant) -> bool {
        self.is_running() && !self.repeat && self.time(now) >= self.length
    }

    pub fn length(&self) -> f32 {
        self.length
    }

    pub fn repeats(&self) -> bool {
        self.repeat
    }

    pub fn run(&mut self, now: Instant) {
        if self.length <= 0.0 {
            return;
        }
        if self.at >= self.length {
            self.at = 0.0;
        }
        self.since = Some(now);
    }

    pub fn stop(&mut self, now: Instant) {
        self.at = self.time(now);
        self.since = None;
    }

    pub fn seek(&mut self, time: f32, now: Instant) {
        self.at = match self.since {
            Some(_) => {
                self.since = Some(now);
                time.clamp(0.0, self.length)
            }
            None => time.max(0.0),
        };
    }

    pub fn set_length(&mut self, length: f32, now: Instant) {
        let time = self.time(now);
        self.length = length.max(0.0);
        self.at = time;
        if self.since.is_some() {
            self.at = time.min(self.length);
            self.since = Some(now);
        }
    }

    pub fn set_repeat(&mut self, repeat: bool, now: Instant) {
        let time = self.time(now);
        self.repeat = repeat;
        self.at = time;
        if self.since.is_some() {
            self.since = Some(now);
        }
    }
}
