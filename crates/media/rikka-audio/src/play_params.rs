use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayParams {
    pub(crate) volume: f32,
    pub(crate) speed: f32,
    pub(crate) pan: f32,
    pub(crate) looping: bool,
    pub(crate) start_at: Option<Duration>,
    pub(crate) paused: bool,
}

impl Default for PlayParams {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayParams {
    pub fn new() -> Self {
        PlayParams {
            volume: 1.0,
            speed: 1.0,
            pan: 0.0,
            looping: false,
            start_at: None,
            paused: false,
        }
    }

    pub fn volume(mut self, volume: f32) -> Self {
        self.volume = volume;
        self
    }

    pub fn speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    pub fn pan(mut self, pan: f32) -> Self {
        self.pan = pan;
        self
    }

    pub fn looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    pub fn start_at(mut self, position: Duration) -> Self {
        self.start_at = Some(position);
        self
    }

    pub fn start_paused(mut self) -> Self {
        self.paused = true;
        self
    }
}
