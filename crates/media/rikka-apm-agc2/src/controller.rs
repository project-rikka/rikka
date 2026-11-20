use rikka_apm_audio::{FRAME, Frame};

use crate::adaptive_gain::{AdaptiveGain, FrameInfo};
use crate::common::{
    ADJACENT_SPEECH_FRAMES_THRESHOLD, SATURATION_PROTECTOR_INITIAL_HEADROOM_DB, db_to_ratio,
    float_s16_to_dbfs,
};
use crate::config::{AdaptiveDigital, Config};
use crate::gain_applier::GainApplier;
use crate::limiter::Limiter;
use crate::noise_level::NoiseFloor;
use crate::saturation::SaturationProtector;
use crate::speech_level::SpeechLevel;
use crate::vad::VoiceActivityDetector;

#[derive(Clone, Debug)]
pub struct GainController {
    channels: usize,
    fixed_gain_db: f32,
    fixed: GainApplier,
    adaptive: Option<Adaptive>,
    limiter: Limiter,
    speech_probability: Option<f32>,
}

#[derive(Clone, Debug)]
struct Adaptive {
    vad: VoiceActivityDetector,
    speech_level: SpeechLevel,
    noise_floor: NoiseFloor,
    noise_level_dbfs: f32,
    saturation: SaturationProtector,
    gain: AdaptiveGain,
}

impl GainController {
    #[must_use]
    pub fn new(config: &Config, channels: usize) -> GainController {
        assert!(
            config.is_valid(),
            "invalid gain controller configuration: {config:?}"
        );
        assert!(
            channels > 0,
            "the gain controller needs at least one channel"
        );
        GainController {
            channels,
            fixed_gain_db: config.fixed_gain_db,
            fixed: GainApplier::new(db_to_ratio(config.fixed_gain_db)),
            adaptive: config
                .adaptive
                .enabled
                .then(|| Adaptive::new(&config.adaptive)),
            limiter: Limiter::new(),
            speech_probability: None,
        }
    }

    pub fn process(
        &mut self,
        frame: &mut Frame,
        speech_probability: Option<f32>,
        input_volume_changed: bool,
    ) {
        let channels = self.channels.min(frame.channels());
        self.speech_probability = speech_probability;
        if let Some(adaptive) = &mut self.adaptive {
            let limiter_envelope_dbfs = float_s16_to_dbfs(self.limiter.last_audio_level());
            let probability = adaptive.process(
                frame,
                channels,
                speech_probability,
                input_volume_changed,
                limiter_envelope_dbfs,
            );
            self.speech_probability = Some(probability);
        }
        self.fixed.apply(frame, channels);
        self.limiter.process(frame, channels);
    }

    #[must_use]
    pub fn speech_probability(&self) -> Option<f32> {
        self.speech_probability
    }

    #[must_use]
    pub fn gain_db(&self) -> f32 {
        self.adaptive
            .as_ref()
            .map_or(0.0, |adaptive| adaptive.gain.gain_db())
            + self.fixed_gain_db
    }

    #[must_use]
    pub fn limiter_gain_db(&self) -> f32 {
        20.0 * self.limiter.last_scaling_factor().log10()
    }

    #[must_use]
    pub fn noise_level_dbfs(&self) -> Option<f32> {
        self.adaptive
            .as_ref()
            .map(|adaptive| adaptive.noise_level_dbfs)
    }
}

impl Adaptive {
    fn new(config: &AdaptiveDigital) -> Adaptive {
        Adaptive {
            vad: VoiceActivityDetector::new(),
            speech_level: SpeechLevel::new(config, ADJACENT_SPEECH_FRAMES_THRESHOLD),
            noise_floor: NoiseFloor::new(),
            noise_level_dbfs: f32::NEG_INFINITY,
            saturation: SaturationProtector::new(
                SATURATION_PROTECTOR_INITIAL_HEADROOM_DB,
                ADJACENT_SPEECH_FRAMES_THRESHOLD,
            ),
            gain: AdaptiveGain::new(config, ADJACENT_SPEECH_FRAMES_THRESHOLD),
        }
    }

    fn process(
        &mut self,
        frame: &mut Frame,
        channels: usize,
        speech_probability: Option<f32>,
        input_volume_changed: bool,
        limiter_envelope_dbfs: f32,
    ) -> f32 {
        if input_volume_changed {
            self.speech_level.reset();
            self.saturation.reset();
        }
        let probability = speech_probability.unwrap_or_else(|| self.vad.analyze(frame.full(0)));
        let (peak_dbfs, rms_dbfs) = levels(frame.full(0));
        self.noise_level_dbfs = self.noise_floor.analyze(frame, channels);
        self.speech_level.update(rms_dbfs, probability);
        let speech_level_dbfs = self.speech_level.level_dbfs();
        self.saturation
            .analyze(probability, peak_dbfs, speech_level_dbfs);
        let info = FrameInfo {
            speech_probability: probability,
            speech_level_dbfs,
            speech_level_reliable: self.speech_level.is_confident(),
            noise_rms_dbfs: self.noise_level_dbfs,
            headroom_db: self.saturation.headroom_db(),
            limiter_envelope_dbfs,
        };
        self.gain.process(&info, frame, channels);
        probability
    }
}

fn levels(samples: &[f32; FRAME]) -> (f32, f32) {
    let mut peak = 0.0f32;
    let mut energy = 0.0f32;
    for &x in samples {
        peak = x.abs().max(peak);
        energy += x * x;
    }
    (
        float_s16_to_dbfs(peak),
        float_s16_to_dbfs((energy / FRAME as f32).sqrt()),
    )
}
