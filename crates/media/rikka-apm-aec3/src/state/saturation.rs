use crate::block::Block;
use crate::filter::SubtractorOutput;

#[derive(Clone, Debug, Default)]
pub(crate) struct SaturationDetector {
    saturated_echo: bool,
}

impl SaturationDetector {
    pub(crate) fn saturated_echo(&self) -> bool {
        self.saturated_echo
    }

    pub(crate) fn update(
        &mut self,
        x: &Block,
        saturated_capture: bool,
        usable_linear_estimate: bool,
        outputs: &[SubtractorOutput],
        echo_path_gain: f32,
    ) {
        self.saturated_echo = false;
        if !saturated_capture {
            return;
        }
        if usable_linear_estimate {
            const SATURATION_THRESHOLD: f32 = 20_000.0;
            self.saturated_echo = outputs.iter().any(|output| {
                output.s_refined_max_abs > SATURATION_THRESHOLD
                    || output.s_coarse_max_abs > SATURATION_THRESHOLD
            });
        } else {
            let mut max_sample = 0.0_f32;
            for ch in 0..x.channels() {
                for sample in x.view(0, ch) {
                    max_sample = max_sample.max(sample.abs());
                }
            }
            const MARGIN: f32 = 10.0;
            let peak_echo_amplitude = max_sample * echo_path_gain * MARGIN;
            self.saturated_echo = peak_echo_amplitude > 32_000.0;
        }
    }
}
