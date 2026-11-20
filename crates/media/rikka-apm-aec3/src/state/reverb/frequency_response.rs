use crate::common::{FFT_LEN_BY_2, SPECTRUM_LEN, Spectrum};

#[derive(Clone, Debug)]
pub(crate) struct ReverbFrequencyResponse {
    use_conservative_tail_frequency_response: bool,
    average_decay: f32,
    tail_response: Spectrum,
}

impl ReverbFrequencyResponse {
    pub(crate) fn new(use_conservative_tail_frequency_response: bool) -> ReverbFrequencyResponse {
        ReverbFrequencyResponse {
            use_conservative_tail_frequency_response,
            average_decay: 0.0,
            tail_response: [0.0; SPECTRUM_LEN],
        }
    }

    pub(crate) fn frequency_response(&self) -> &Spectrum {
        &self.tail_response
    }

    pub(crate) fn update(
        &mut self,
        frequency_response: &[Spectrum],
        filter_delay_blocks: i32,
        linear_filter_quality: Option<f32>,
        stationary_block: bool,
    ) {
        if stationary_block {
            return;
        }
        let Some(quality) = linear_filter_quality else {
            return;
        };
        let tail = &frequency_response[frequency_response.len() - 1];
        let direct_path = &frequency_response[filter_delay_blocks as usize];
        let average_decay = average_decay_within_filter(direct_path, tail);
        let smoothing = 0.2 * quality;
        self.average_decay += smoothing * (average_decay - self.average_decay);
        for (response, direct) in self.tail_response.iter_mut().zip(direct_path) {
            *response = direct * self.average_decay;
        }
        if self.use_conservative_tail_frequency_response {
            for (response, tail) in self.tail_response.iter_mut().zip(tail) {
                *response = tail.max(*response);
            }
        }
        for k in 1..FFT_LEN_BY_2 {
            let avg_neighbour = 0.5 * (self.tail_response[k - 1] + self.tail_response[k + 1]);
            self.tail_response[k] = self.tail_response[k].max(avg_neighbour);
        }
    }
}

fn average_decay_within_filter(direct_path: &Spectrum, tail: &Spectrum) -> f32 {
    const SKIP_BINS: usize = 1;
    let direct_path_energy = crate::common::sum(&direct_path[SKIP_BINS..]);
    if direct_path_energy == 0.0 {
        return 0.0;
    }
    crate::common::sum(&tail[SKIP_BINS..]) / direct_path_energy
}
