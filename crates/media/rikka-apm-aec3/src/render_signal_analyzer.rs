use crate::common::{FFT_LEN_BY_2, Spectrum};
use crate::render::RenderBuffer;

const COUNTER_THRESHOLD: usize = 5;

#[derive(Clone, Debug)]
pub(crate) struct RenderSignalAnalyzer {
    strong_peak_freeze_duration: usize,
    narrow_band_counters: [usize; FFT_LEN_BY_2 - 1],
    narrow_peak_band: Option<usize>,
    narrow_peak_counter: usize,
}

impl RenderSignalAnalyzer {
    pub(crate) fn new(refined_length_blocks: usize) -> RenderSignalAnalyzer {
        RenderSignalAnalyzer {
            strong_peak_freeze_duration: refined_length_blocks,
            narrow_band_counters: [0; FFT_LEN_BY_2 - 1],
            narrow_peak_band: None,
            narrow_peak_counter: 0,
        }
    }

    pub(crate) fn update(&mut self, render: &RenderBuffer, delay_partitions: Option<usize>) {
        self.identify_small_narrow_band_regions(render, delay_partitions);
        self.identify_strong_narrow_band_component(render);
    }

    pub(crate) fn poor_signal_excitation(&self) -> bool {
        self.narrow_band_counters
            .iter()
            .any(|counter| *counter > 10)
    }

    pub(crate) fn narrow_peak_band(&self) -> Option<usize> {
        self.narrow_peak_band
    }

    pub(crate) fn mask_regions_around_narrow_bands(&self, v: &mut Spectrum) {
        if self.narrow_band_counters[0] > COUNTER_THRESHOLD {
            v[0] = 0.0;
            v[1] = 0.0;
        }
        for k in 2..FFT_LEN_BY_2 - 1 {
            if self.narrow_band_counters[k - 1] > COUNTER_THRESHOLD {
                v[k - 2..=k + 2].fill(0.0);
            }
        }
        if self.narrow_band_counters[FFT_LEN_BY_2 - 2] > COUNTER_THRESHOLD {
            v[FFT_LEN_BY_2] = 0.0;
            v[FFT_LEN_BY_2 - 1] = 0.0;
        }
    }

    fn identify_small_narrow_band_regions(
        &mut self,
        render: &RenderBuffer,
        delay_partitions: Option<usize>,
    ) {
        let Some(delay_partitions) = delay_partitions else {
            self.narrow_band_counters = [0; FFT_LEN_BY_2 - 1];
            return;
        };
        let mut channel_counters = [0_usize; FFT_LEN_BY_2 - 1];
        for x2 in render.spectrum(delay_partitions as isize) {
            for k in 1..FFT_LEN_BY_2 {
                if x2[k] > 3.0 * x2[k - 1].max(x2[k + 1]) {
                    channel_counters[k - 1] += 1;
                }
            }
        }
        for (counter, channel) in self.narrow_band_counters.iter_mut().zip(channel_counters) {
            *counter = if channel > 0 { *counter + 1 } else { 0 };
        }
    }

    fn identify_strong_narrow_band_component(&mut self, render: &RenderBuffer) {
        if self.narrow_peak_band.is_some() {
            self.narrow_peak_counter += 1;
            if self.narrow_peak_counter > self.strong_peak_freeze_duration {
                self.narrow_peak_band = None;
            }
        }
        let x_latest = render.block(0);
        let mut max_peak_level = 0.0_f32;
        for channel in 0..x_latest.channels() {
            let x2_latest = &render.spectrum(0)[channel];
            let peak_bin = first_max_index(x2_latest);
            let mut non_peak_power = 0.0_f32;
            for value in &x2_latest[peak_bin.saturating_sub(14)..peak_bin.saturating_sub(4)] {
                non_peak_power = value.max(non_peak_power);
            }
            for value in x2_latest.iter().take(peak_bin + 15).skip(peak_bin + 5) {
                non_peak_power = value.max(non_peak_power);
            }
            let mut max_abs = max_abs_of(x_latest.view(0, channel));
            if x_latest.bands() > 1 {
                max_abs = max_abs.max(max_abs_of(x_latest.view(1, channel)));
            }
            let peak_level = x2_latest[peak_bin];
            if peak_bin > 0
                && max_abs > 100.0
                && peak_level > 100.0 * non_peak_power
                && peak_level > max_peak_level
            {
                max_peak_level = peak_level;
                self.narrow_peak_band = Some(peak_bin);
                self.narrow_peak_counter = 0;
            }
        }
    }
}

fn first_max_index(values: &[f32]) -> usize {
    let mut best = 0;
    for (index, value) in values.iter().enumerate() {
        if *value > values[best] {
            best = index;
        }
    }
    best
}

fn max_abs_of(values: &[f32]) -> f32 {
    let mut min = values[0];
    let mut max = values[0];
    for value in &values[1..] {
        if *value < min {
            min = *value;
        }
        if *value >= max {
            max = *value;
        }
    }
    min.abs().max(max.abs())
}
