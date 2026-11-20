use rikka_apm_audio::Frame;

use crate::common::{
    FRAME_DURATION_MS, LIMITER_THRESHOLD_FOR_AGC_GAIN_DBFS, VAD_CONFIDENCE_THRESHOLD, db_to_ratio,
};
use crate::config::AdaptiveDigital;
use crate::gain_applier::GainApplier;

#[derive(Clone, Copy, Debug)]
pub(crate) struct FrameInfo {
    pub(crate) speech_probability: f32,
    pub(crate) speech_level_dbfs: f32,
    pub(crate) speech_level_reliable: bool,
    pub(crate) noise_rms_dbfs: f32,
    pub(crate) headroom_db: f32,
    pub(crate) limiter_envelope_dbfs: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct AdaptiveGain {
    applier: GainApplier,
    config: AdaptiveDigital,
    adjacent_speech_frames_threshold: i32,
    max_gain_change_db_per_10ms: f32,
    frames_to_gain_increase_allowed: i32,
    last_gain_db: f32,
}

impl AdaptiveGain {
    pub(crate) fn new(
        config: &AdaptiveDigital,
        adjacent_speech_frames_threshold: i32,
    ) -> AdaptiveGain {
        AdaptiveGain {
            applier: GainApplier::new(db_to_ratio(config.initial_gain_db)),
            config: *config,
            adjacent_speech_frames_threshold,
            max_gain_change_db_per_10ms: config.max_gain_change_db_per_second
                * FRAME_DURATION_MS as f32
                / 1000.0,
            frames_to_gain_increase_allowed: adjacent_speech_frames_threshold,
            last_gain_db: config.initial_gain_db,
        }
    }

    pub(crate) fn gain_db(&self) -> f32 {
        self.last_gain_db
    }

    pub(crate) fn process(&mut self, info: &FrameInfo, frame: &mut Frame, channels: usize) {
        let input_level_dbfs = info.speech_level_dbfs + info.headroom_db;
        let target_gain_db = limit_gain_by_low_confidence(
            limit_gain_by_noise(
                gain_for_level(input_level_dbfs, &self.config),
                info.noise_rms_dbfs,
                self.config.max_output_noise_level_dbfs,
            ),
            self.last_gain_db,
            info.limiter_envelope_dbfs,
            info.speech_level_reliable,
        );

        let mut first_confident_speech_frame = false;
        if info.speech_probability < VAD_CONFIDENCE_THRESHOLD {
            self.frames_to_gain_increase_allowed = self.adjacent_speech_frames_threshold;
        } else if self.frames_to_gain_increase_allowed > 0 {
            self.frames_to_gain_increase_allowed -= 1;
            first_confident_speech_frame = self.frames_to_gain_increase_allowed == 0;
        }

        let gain_increase_allowed = self.frames_to_gain_increase_allowed == 0;
        let mut max_gain_increase_db = self.max_gain_change_db_per_10ms;
        if first_confident_speech_frame {
            max_gain_increase_db *= self.adjacent_speech_frames_threshold as f32;
        }

        let change_db = gain_change_this_frame_db(
            target_gain_db,
            self.last_gain_db,
            gain_increase_allowed,
            self.max_gain_change_db_per_10ms,
            max_gain_increase_db,
        );
        if change_db != 0.0 {
            self.applier
                .set_factor(db_to_ratio(self.last_gain_db + change_db));
        }
        self.applier.apply(frame, channels);
        self.last_gain_db += change_db;
    }
}

fn gain_for_level(input_level_dbfs: f32, config: &AdaptiveDigital) -> f32 {
    if input_level_dbfs < -(config.headroom_db + config.max_gain_db) {
        return config.max_gain_db;
    }
    if input_level_dbfs < -config.headroom_db {
        return -config.headroom_db - input_level_dbfs;
    }
    0.0
}

fn limit_gain_by_noise(
    target_gain_db: f32,
    noise_level_dbfs: f32,
    max_output_noise_level_dbfs: f32,
) -> f32 {
    let max_allowed_gain_db = max_output_noise_level_dbfs - noise_level_dbfs;
    target_gain_db.min(max_allowed_gain_db.max(0.0))
}

fn limit_gain_by_low_confidence(
    target_gain_db: f32,
    last_gain_db: f32,
    limiter_level_dbfs: f32,
    confident: bool,
) -> f32 {
    if confident || limiter_level_dbfs <= LIMITER_THRESHOLD_FOR_AGC_GAIN_DBFS {
        return target_gain_db;
    }
    let level_before_gain_dbfs = limiter_level_dbfs - last_gain_db;
    let new_target_gain_db =
        (LIMITER_THRESHOLD_FOR_AGC_GAIN_DBFS - level_before_gain_dbfs).max(0.0);
    new_target_gain_db.min(target_gain_db)
}

fn gain_change_this_frame_db(
    target_gain_db: f32,
    last_gain_db: f32,
    gain_increase_allowed: bool,
    max_decrease_db: f32,
    max_increase_db: f32,
) -> f32 {
    let mut difference_db = target_gain_db - last_gain_db;
    if !gain_increase_allowed {
        difference_db = difference_db.min(0.0);
    }
    difference_db.clamp(-max_decrease_db, max_increase_db)
}
