use crate::common::BLOCK_LEN;
use crate::filter::subtractor_output::SubtractorOutput;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Convergence {
    pub(crate) any_filter_converged: bool,
    pub(crate) any_coarse_filter_converged: bool,
    pub(crate) all_filters_diverged: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct SubtractorOutputAnalyzer {
    filters_converged: Vec<bool>,
}

impl SubtractorOutputAnalyzer {
    pub(crate) fn new(capture_channels: usize) -> SubtractorOutputAnalyzer {
        SubtractorOutputAnalyzer {
            filters_converged: vec![false; capture_channels],
        }
    }

    pub(crate) fn converged_filters(&self) -> &[bool] {
        &self.filters_converged
    }

    pub(crate) fn handle_echo_path_change(&mut self) {
        self.filters_converged.fill(false);
    }

    pub(crate) fn update(&mut self, outputs: &[SubtractorOutput]) -> Convergence {
        const CONVERGENCE_THRESHOLD: f32 = (50 * 50 * BLOCK_LEN) as f32;
        const CONVERGENCE_THRESHOLD_LOW_LEVEL: f32 = (20 * 20 * BLOCK_LEN) as f32;
        let mut result = Convergence {
            any_filter_converged: false,
            any_coarse_filter_converged: false,
            all_filters_diverged: true,
        };
        for (converged, output) in self.filters_converged.iter_mut().zip(outputs) {
            let (y2, e2_refined, e2_coarse) = (output.y2, output.e2_refined, output.e2_coarse);
            let refined_converged = e2_refined < 0.5 * y2 && y2 > CONVERGENCE_THRESHOLD;
            let coarse_converged_strict = e2_coarse < 0.05 * y2 && y2 > CONVERGENCE_THRESHOLD;
            let coarse_converged_relaxed =
                e2_coarse < 0.3 * y2 && y2 > CONVERGENCE_THRESHOLD_LOW_LEVEL;
            let min_e2 = e2_refined.min(e2_coarse);
            let diverged = min_e2 > 1.5 * y2 && y2 > 30.0 * 30.0 * BLOCK_LEN as f32;
            *converged = refined_converged || coarse_converged_strict;
            result.any_filter_converged |= *converged;
            result.any_coarse_filter_converged |= coarse_converged_relaxed;
            result.all_filters_diverged &= diverged;
        }
        result
    }
}
