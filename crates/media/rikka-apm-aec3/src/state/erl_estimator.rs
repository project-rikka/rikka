use crate::common::Spectrum;

const MIN_ERL: f32 = 0.01;
const MAX_ERL: f32 = 1000.0;
const X2_MIN: f32 = 44_015_068.0;

#[derive(Clone, Debug)]
pub(crate) struct ErlEstimator {
    startup_phase_length_blocks: usize,
    erl_time_domain: f32,
    hold_counter_time_domain: i32,
    blocks_since_reset: usize,
}

impl ErlEstimator {
    pub(crate) fn new(startup_phase_length_blocks: usize) -> ErlEstimator {
        ErlEstimator {
            startup_phase_length_blocks,
            erl_time_domain: MAX_ERL,
            hold_counter_time_domain: 0,
            blocks_since_reset: 0,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.blocks_since_reset = 0;
    }

    pub(crate) fn erl_time_domain(&self) -> f32 {
        self.erl_time_domain
    }

    pub(crate) fn update(
        &mut self,
        converged_filters: &[bool],
        render_spectra: &[Spectrum],
        capture_spectra: &[Spectrum],
    ) {
        self.blocks_since_reset += 1;
        if self.blocks_since_reset < self.startup_phase_length_blocks {
            return;
        }
        let Some(first_converged) = converged_filters.iter().position(|converged| *converged)
        else {
            return;
        };

        let mut y2 = capture_spectra[0];
        if converged_filters.len() > 1 {
            y2 = capture_spectra[first_converged];
            let converged = capture_spectra
                .iter()
                .zip(converged_filters)
                .skip(first_converged + 1);
            for (spectrum, _) in converged.filter(|(_, converged)| **converged) {
                max_into(&mut y2, spectrum);
            }
        }
        let mut x2 = render_spectra[0];
        for spectrum in &render_spectra[1..] {
            max_into(&mut x2, spectrum);
        }

        let x2_sum = crate::common::sum(&x2);
        if x2_sum > X2_MIN * x2.len() as f32 {
            let new_erl = crate::common::sum(&y2) / x2_sum;
            if new_erl < self.erl_time_domain {
                self.hold_counter_time_domain = 1000;
                self.erl_time_domain += 0.1 * (new_erl - self.erl_time_domain);
                self.erl_time_domain = self.erl_time_domain.max(MIN_ERL);
            }
        }
        self.hold_counter_time_domain = self.hold_counter_time_domain.saturating_sub(1);
        if self.hold_counter_time_domain <= 0 {
            self.erl_time_domain = MAX_ERL.min(2.0 * self.erl_time_domain);
        }
    }
}

fn max_into(max: &mut Spectrum, spectrum: &Spectrum) {
    for (max, value) in max.iter_mut().zip(spectrum) {
        *max = max.max(*value);
    }
}
