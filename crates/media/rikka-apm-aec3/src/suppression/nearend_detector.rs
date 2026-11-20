use crate::common::Spectrum;
use crate::config::{DominantNearendDetection, Suppressor};
use crate::suppression::moving_average::MovingAverage;

#[derive(Clone, Copy, Debug)]
pub(crate) struct NearendInput<'a> {
    pub(crate) nearend_spectrum: &'a [Spectrum],
    pub(crate) residual_echo_spectrum: &'a [Spectrum],
    pub(crate) comfort_noise_spectrum: &'a [Spectrum],
    pub(crate) initial_state: bool,
}

#[derive(Clone, Debug)]
pub(crate) enum NearendDetector {
    Dominant(DominantNearendDetector),
    Subband(SubbandNearendDetector),
}

impl NearendDetector {
    pub(crate) fn new(config: &Suppressor, capture_channels: usize) -> NearendDetector {
        if config.use_subband_nearend_detection {
            NearendDetector::Subband(SubbandNearendDetector::new(config, capture_channels))
        } else {
            NearendDetector::Dominant(DominantNearendDetector::new(
                &config.dominant_nearend_detection,
                capture_channels,
            ))
        }
    }

    pub(crate) fn is_nearend_state(&self) -> bool {
        match self {
            NearendDetector::Dominant(detector) => detector.nearend_state,
            NearendDetector::Subband(detector) => detector.nearend_state,
        }
    }

    pub(crate) fn update(&mut self, input: &NearendInput<'_>) {
        match self {
            NearendDetector::Dominant(detector) => detector.update(input),
            NearendDetector::Subband(detector) => detector.update(input),
        }
    }
}

fn low_frequency_energy(spectrum: &Spectrum) -> f32 {
    crate::common::sum(&spectrum[1..16])
}

#[derive(Clone, Debug)]
pub(crate) struct DominantNearendDetector {
    config: DominantNearendDetection,
    nearend_state: bool,
    trigger_counters: Vec<i32>,
    hold_counters: Vec<i32>,
}

impl DominantNearendDetector {
    fn new(config: &DominantNearendDetection, capture_channels: usize) -> DominantNearendDetector {
        DominantNearendDetector {
            config: config.clone(),
            nearend_state: false,
            trigger_counters: vec![0; capture_channels],
            hold_counters: vec![0; capture_channels],
        }
    }

    fn update(&mut self, input: &NearendInput<'_>) {
        let config = &self.config;
        self.nearend_state = false;
        for ch in 0..self.trigger_counters.len() {
            let ne_sum = low_frequency_energy(&input.nearend_spectrum[ch]);
            let echo_sum = low_frequency_energy(&input.residual_echo_spectrum[ch]);
            let noise_sum = low_frequency_energy(&input.comfort_noise_spectrum[ch]);
            if (!input.initial_state || config.use_during_initial_phase)
                && echo_sum < config.enr_threshold * ne_sum
                && ne_sum > config.snr_threshold * noise_sum
            {
                self.trigger_counters[ch] += 1;
                if self.trigger_counters[ch] >= config.trigger_threshold {
                    self.hold_counters[ch] = config.hold_duration;
                    self.trigger_counters[ch] = config.trigger_threshold;
                }
            } else {
                self.trigger_counters[ch] = (self.trigger_counters[ch] - 1).max(0);
            }
            if echo_sum > config.enr_exit_threshold * ne_sum
                && echo_sum > config.snr_threshold * noise_sum
            {
                self.hold_counters[ch] = 0;
            }
            self.hold_counters[ch] = (self.hold_counters[ch] - 1).max(0);
            self.nearend_state = self.nearend_state || self.hold_counters[ch] > 0;
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SubbandNearendDetector {
    low1: usize,
    high1: usize,
    low2: usize,
    high2: usize,
    nearend_threshold: f32,
    snr_threshold: f32,
    smoothers: Vec<MovingAverage>,
    one_over_subband_length1: f32,
    one_over_subband_length2: f32,
    nearend_state: bool,
}

impl SubbandNearendDetector {
    fn new(config: &Suppressor, capture_channels: usize) -> SubbandNearendDetector {
        let config = &config.subband_nearend_detection;
        let last = crate::common::SPECTRUM_LEN - 1;
        let (low1, low2) = (config.subband1.low.min(last), config.subband2.low.min(last));
        let (high1, high2) = (
            config.subband1.high.clamp(low1, last),
            config.subband2.high.clamp(low2, last),
        );
        SubbandNearendDetector {
            low1,
            high1,
            low2,
            high2,
            nearend_threshold: config.nearend_threshold,
            snr_threshold: config.snr_threshold,
            smoothers: vec![MovingAverage::new(config.nearend_average_blocks); capture_channels],
            one_over_subband_length1: 1.0 / (high1 - low1 + 1) as f32,
            one_over_subband_length2: 1.0 / (high2 - low2 + 1) as f32,
            nearend_state: false,
        }
    }

    fn update(&mut self, input: &NearendInput<'_>) {
        self.nearend_state = false;
        for (ch, smoother) in self.smoothers.iter_mut().enumerate() {
            let noise = &input.comfort_noise_spectrum[ch];
            let mut nearend = [0.0; crate::common::SPECTRUM_LEN];
            smoother.average(&input.nearend_spectrum[ch], &mut nearend);
            let sum = crate::common::sum;
            let noise_power = sum(&noise[self.low1..=self.high1]) * self.one_over_subband_length1;
            let nearend_power_subband1 =
                sum(&nearend[self.low1..=self.high1]) * self.one_over_subband_length1;
            let nearend_power_subband2 =
                sum(&nearend[self.low2..=self.high2]) * self.one_over_subband_length2;
            self.nearend_state = self.nearend_state
                || (nearend_power_subband1 < self.nearend_threshold * nearend_power_subband2
                    && nearend_power_subband1 > self.snr_threshold * noise_power);
        }
    }
}
