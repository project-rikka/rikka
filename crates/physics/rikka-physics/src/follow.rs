const MAX_JUMP: f32 = 1.0;
const MAX_BACKWARD_JITTER: f32 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pace {
    Playing,
    Stopped { idle: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    Seek,
    Advance(f32),
}

#[must_use]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Follow {
    max_jump: f32,
    max_backward_jitter: f32,
}

impl Default for Follow {
    fn default() -> Self {
        Self {
            max_jump: MAX_JUMP,
            max_backward_jitter: MAX_BACKWARD_JITTER,
        }
    }
}

impl Follow {
    pub fn max_jump(mut self, seconds: f32) -> Self {
        self.max_jump = seconds;
        self
    }

    pub fn max_backward_jitter(mut self, seconds: f32) -> Self {
        self.max_backward_jitter = seconds;
        self
    }

    #[must_use]
    pub fn step(self, followed: Option<f32>, time: f32, pace: Pace) -> Step {
        let Some(followed) = followed else {
            return Step::Seek;
        };
        let ahead = time - followed;
        match pace {
            Pace::Playing if (-self.max_backward_jitter..=self.max_jump).contains(&ahead) => {
                Step::Advance(ahead.max(0.0))
            }
            Pace::Stopped { idle } if ahead.abs() <= self.max_jump => Step::Advance(idle),
            _ => Step::Seek,
        }
    }
}
