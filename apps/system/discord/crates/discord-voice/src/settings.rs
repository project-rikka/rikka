use std::time::Duration;

use serde::{Deserialize, Serialize};

pub const MAX_VOLUME: f32 = 2.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceSettings {
    pub input: Option<String>,
    pub output: Option<String>,
    pub input_volume: f32,
    pub output_volume: f32,
    pub mode: InputMode,
    pub noise_suppression: NoiseSuppression,
    pub echo_cancellation: bool,
    pub automatic_gain: bool,
}

impl Default for VoiceSettings {
    fn default() -> VoiceSettings {
        VoiceSettings {
            input: None,
            output: None,
            input_volume: 1.0,
            output_volume: 1.0,
            mode: InputMode::default(),
            noise_suppression: NoiseSuppression::default(),
            echo_cancellation: true,
            automatic_gain: true,
        }
    }
}

impl VoiceSettings {
    pub(crate) fn input_gain(&self) -> f32 {
        volume(self.input_volume)
    }

    pub(crate) fn output_gain(&self) -> f32 {
        volume(self.output_volume)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NoiseSuppression {
    Off,
    Standard,
    #[default]
    Krisp,
}

impl NoiseSuppression {
    pub const ALL: [NoiseSuppression; 3] = [
        NoiseSuppression::Off,
        NoiseSuppression::Standard,
        NoiseSuppression::Krisp,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum InputMode {
    VoiceActivity { threshold_db: f32, automatic: bool },
    PushToTalk { release: Duration },
}

impl Default for InputMode {
    fn default() -> InputMode {
        InputMode::VoiceActivity {
            threshold_db: -50.0,
            automatic: true,
        }
    }
}

pub(crate) fn volume(volume: f32) -> f32 {
    if volume.is_nan() {
        return 1.0;
    }
    volume.clamp(0.0, MAX_VOLUME)
}
