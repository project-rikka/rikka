use std::time::Duration;

use crate::input::vad::Vad;
use crate::settings::InputMode;

#[derive(Debug)]
pub(crate) enum Gate {
    Voice(Vad),
    PushToTalk {
        release: Duration,
        released: Duration,
    },
}

impl Gate {
    pub(crate) fn new(mode: InputMode) -> Gate {
        match mode {
            InputMode::VoiceActivity {
                automatic: true, ..
            } => Gate::Voice(Vad::automatic()),
            InputMode::VoiceActivity {
                threshold_db,
                automatic: false,
            } => Gate::Voice(Vad::fixed(threshold_db)),
            InputMode::PushToTalk { release } => Gate::PushToTalk {
                release,
                released: Duration::MAX,
            },
        }
    }

    pub(crate) fn update(
        &mut self,
        level_db: f32,
        probability: Option<f32>,
        pressed: bool,
        frame: Duration,
    ) -> bool {
        match self {
            Gate::Voice(vad) => vad.update(level_db, probability, frame),
            Gate::PushToTalk { release, released } => {
                if pressed {
                    *released = Duration::ZERO;
                    return true;
                }
                *released = released.saturating_add(frame);
                *released <= *release
            }
        }
    }
}
