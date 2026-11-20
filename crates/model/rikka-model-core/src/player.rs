use crate::animation::AnimationId;
use crate::model::Model;
use crate::pose::Pose;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Repeat {
    Once,
    #[default]
    Forever,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimationPlayer {
    animation: Option<AnimationId>,
    duration: f32,
    time: f32,
    speed: f32,
    repeat: Repeat,
    playing: bool,
}

impl Default for AnimationPlayer {
    fn default() -> Self {
        Self {
            animation: None,
            duration: 0.0,
            time: 0.0,
            speed: 1.0,
            repeat: Repeat::default(),
            playing: false,
        }
    }
}

impl AnimationPlayer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn play(&mut self, model: &Model, animation: AnimationId) {
        self.animation = Some(animation);
        self.duration = model.animation(animation).duration();
        self.time = 0.0;
        self.playing = true;
    }

    pub fn stop(&mut self) {
        self.animation = None;
        self.playing = false;
        self.time = 0.0;
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    pub fn resume(&mut self) {
        self.playing = self.animation.is_some();
    }

    pub fn seek(&mut self, time: f32) {
        self.time = time.clamp(0.0, self.duration);
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    pub fn set_repeat(&mut self, repeat: Repeat) {
        self.repeat = repeat;
    }

    pub fn advance(&mut self, delta: f32) {
        if !self.playing || self.duration <= 0.0 {
            return;
        }
        let time = self.time + delta * self.speed;
        self.time = match self.repeat {
            Repeat::Forever => time.rem_euclid(self.duration),
            Repeat::Once => {
                let clamped = time.clamp(0.0, self.duration);
                self.playing = clamped > 0.0 && clamped < self.duration;
                clamped
            }
        };
    }

    #[must_use]
    pub fn animation(&self) -> Option<AnimationId> {
        self.animation
    }

    #[must_use]
    pub fn time(&self) -> f32 {
        self.time
    }

    #[must_use]
    pub fn duration(&self) -> f32 {
        self.duration
    }

    #[must_use]
    pub fn speed(&self) -> f32 {
        self.speed
    }

    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn apply(&self, model: &Model, pose: &mut Pose) {
        let Some(animation) = self.animation else {
            return;
        };
        model.animation(animation).apply(self.time, pose);
    }
}
