use crate::common::{
    FRAME_DURATION_MS, LEVEL_ESTIMATOR_LEAK_FACTOR, LEVEL_ESTIMATOR_TIME_TO_CONFIDENCE_MS,
    SATURATION_PROTECTOR_INITIAL_HEADROOM_DB, VAD_CONFIDENCE_THRESHOLD,
};
use crate::config::AdaptiveDigital;

#[derive(Clone, Copy, Debug)]
struct State {
    time_to_confidence_ms: i32,
    numerator: f32,
    denominator: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct SpeechLevel {
    initial_level_dbfs: f32,
    adjacent_speech_frames_threshold: i32,
    preliminary: State,
    reliable: State,
    level_dbfs: f32,
    confident: bool,
    adjacent_speech_frames: i32,
}

impl SpeechLevel {
    pub(crate) fn new(
        config: &AdaptiveDigital,
        adjacent_speech_frames_threshold: i32,
    ) -> SpeechLevel {
        let initial_level_dbfs = clamp_level(
            -SATURATION_PROTECTOR_INITIAL_HEADROOM_DB - config.initial_gain_db - config.headroom_db,
        );
        let state = initial_state(initial_level_dbfs);
        SpeechLevel {
            initial_level_dbfs,
            adjacent_speech_frames_threshold,
            preliminary: state,
            reliable: state,
            level_dbfs: initial_level_dbfs,
            confident: false,
            adjacent_speech_frames: 0,
        }
    }

    pub(crate) fn level_dbfs(&self) -> f32 {
        self.level_dbfs
    }

    pub(crate) fn is_confident(&self) -> bool {
        self.confident
    }

    pub(crate) fn reset(&mut self) {
        self.preliminary = initial_state(self.initial_level_dbfs);
        self.reliable = initial_state(self.initial_level_dbfs);
        self.level_dbfs = self.initial_level_dbfs;
        self.adjacent_speech_frames = 0;
    }

    pub(crate) fn update(&mut self, rms_dbfs: f32, speech_probability: f32) {
        if speech_probability < VAD_CONFIDENCE_THRESHOLD {
            if self.adjacent_speech_frames_threshold > 1 {
                if self.adjacent_speech_frames >= self.adjacent_speech_frames_threshold {
                    self.reliable = self.preliminary;
                } else if self.adjacent_speech_frames > 0 {
                    self.preliminary = self.reliable;
                }
            }
            self.adjacent_speech_frames = 0;
        } else {
            self.adjacent_speech_frames += 1;
            let buffer_is_full = self.preliminary.time_to_confidence_ms == 0;
            if !buffer_is_full {
                self.preliminary.time_to_confidence_ms -= FRAME_DURATION_MS;
            }
            let leak_factor = if buffer_is_full {
                LEVEL_ESTIMATOR_LEAK_FACTOR
            } else {
                1.0
            };
            self.preliminary.numerator =
                self.preliminary.numerator * leak_factor + rms_dbfs * speech_probability;
            self.preliminary.denominator =
                self.preliminary.denominator * leak_factor + speech_probability;
            let level_dbfs = self.preliminary.numerator / self.preliminary.denominator;
            if self.adjacent_speech_frames >= self.adjacent_speech_frames_threshold {
                self.level_dbfs = clamp_level(level_dbfs);
            }
        }
        self.update_confidence();
    }

    fn update_confidence(&mut self) {
        if self.adjacent_speech_frames_threshold == 1 {
            self.confident = self.preliminary.time_to_confidence_ms == 0;
            return;
        }
        self.confident = self.reliable.time_to_confidence_ms == 0
            || (self.adjacent_speech_frames >= self.adjacent_speech_frames_threshold
                && self.preliminary.time_to_confidence_ms == 0);
    }
}

fn initial_state(level_dbfs: f32) -> State {
    State {
        time_to_confidence_ms: LEVEL_ESTIMATOR_TIME_TO_CONFIDENCE_MS,
        numerator: level_dbfs,
        denominator: 1.0,
    }
}

fn clamp_level(level_dbfs: f32) -> f32 {
    level_dbfs.clamp(-90.0, 30.0)
}
