mod fullband;
mod signal_dependent;
mod subband;

use crate::common::Spectrum;
use crate::config::Config;
use crate::render::RenderBuffer;
use fullband::FullbandErleEstimator;
use signal_dependent::{SignalDependentErleEstimator, SignalDependentInput};
use subband::SubbandErleEstimator;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ErleInput<'a> {
    pub(crate) render: &'a RenderBuffer,
    pub(crate) filter_frequency_responses: &'a [Vec<Spectrum>],
    pub(crate) avg_render_spectrum_with_reverb: &'a Spectrum,
    pub(crate) capture_spectra: &'a [Spectrum],
    pub(crate) subtractor_spectra: &'a [Spectrum],
    pub(crate) converged_filters: &'a [bool],
}

#[derive(Clone, Debug)]
pub(crate) struct ErleEstimator {
    startup_phase_length_blocks: usize,
    fullband: FullbandErleEstimator,
    subband: SubbandErleEstimator,
    signal_dependent: Option<SignalDependentErleEstimator>,
    blocks_since_reset: usize,
}

impl ErleEstimator {
    pub(crate) fn new(
        startup_phase_length_blocks: usize,
        config: &Config,
        capture_channels: usize,
    ) -> ErleEstimator {
        let mut estimator = ErleEstimator {
            startup_phase_length_blocks,
            fullband: FullbandErleEstimator::new(&config.erle, capture_channels),
            subband: SubbandErleEstimator::new(config, capture_channels),
            signal_dependent: (config.erle.num_sections > 1)
                .then(|| SignalDependentErleEstimator::new(config, capture_channels)),
            blocks_since_reset: 0,
        };
        estimator.reset(true);
        estimator
    }

    pub(crate) fn reset(&mut self, delay_change: bool) {
        self.fullband.reset();
        self.subband.reset();
        if let Some(signal_dependent) = &mut self.signal_dependent {
            signal_dependent.reset();
        }
        if delay_change {
            self.blocks_since_reset = 0;
        }
    }

    pub(crate) fn erle(&self, onset_compensated: bool) -> &[Spectrum] {
        match &self.signal_dependent {
            Some(signal_dependent) => signal_dependent.erle(onset_compensated),
            None => self.subband.erle(onset_compensated),
        }
    }

    pub(crate) fn erle_unbounded(&self) -> &[Spectrum] {
        match &self.signal_dependent {
            Some(signal_dependent) => signal_dependent.erle(false),
            None => self.subband.erle_unbounded(),
        }
    }

    pub(crate) fn fullband_erle_log2(&self) -> f32 {
        self.fullband.fullband_erle_log2()
    }

    pub(crate) fn inst_linear_quality_estimates(&self) -> &[Option<f32>] {
        self.fullband.inst_linear_quality_estimates()
    }

    pub(crate) fn update(&mut self, input: &ErleInput<'_>) {
        self.blocks_since_reset += 1;
        if self.blocks_since_reset < self.startup_phase_length_blocks {
            return;
        }
        let x2_reverb = input.avg_render_spectrum_with_reverb;
        self.subband.update(
            x2_reverb,
            input.capture_spectra,
            input.subtractor_spectra,
            input.converged_filters,
        );
        if let Some(signal_dependent) = &mut self.signal_dependent {
            signal_dependent.update(&SignalDependentInput {
                render: input.render,
                filter_frequency_responses: input.filter_frequency_responses,
                x2: x2_reverb,
                y2: input.capture_spectra,
                e2: input.subtractor_spectra,
                average_erle: self.subband.erle(false),
                average_erle_onset_compensated: self.subband.erle(true),
                converged_filters: input.converged_filters,
            });
        }
        self.fullband.update(
            x2_reverb,
            input.capture_spectra,
            input.subtractor_spectra,
            input.converged_filters,
        );
    }
}
