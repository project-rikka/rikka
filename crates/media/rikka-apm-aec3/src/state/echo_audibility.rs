use crate::common::{BLOCK_LEN, Spectrum};
use crate::render::RenderBuffer;
use crate::state::stationarity::StationarityEstimator;

#[derive(Clone, Debug)]
pub(crate) struct EchoAudibility {
    render_spectrum_write_prev: Option<usize>,
    render_block_write_prev: usize,
    non_zero_render_seen: bool,
    use_render_stationarity_at_init: bool,
    render_stationarity: StationarityEstimator,
}

impl EchoAudibility {
    pub(crate) fn new(use_render_stationarity_at_init: bool) -> EchoAudibility {
        EchoAudibility {
            render_spectrum_write_prev: None,
            render_block_write_prev: 0,
            non_zero_render_seen: false,
            use_render_stationarity_at_init,
            render_stationarity: StationarityEstimator::new(),
        }
    }

    pub(crate) fn update(
        &mut self,
        render: &RenderBuffer,
        average_reverb: &Spectrum,
        delay_blocks: i32,
        external_delay_seen: bool,
    ) {
        self.update_render_noise_estimator(render, external_delay_seen);
        if external_delay_seen || self.use_render_stationarity_at_init {
            let spectra = &render.spectra;
            let idx_at_delay = spectra.offset(spectra.read, delay_blocks as isize);
            let num_lookahead = (render.headroom() as i32 - delay_blocks + 1).max(0);
            self.render_stationarity.update_stationarity_flags(
                spectra,
                average_reverb,
                idx_at_delay,
                num_lookahead,
            );
        }
    }

    pub(crate) fn residual_echo_scaling(
        &self,
        filter_has_had_time_to_converge: bool,
        residual_scaling: &mut Spectrum,
    ) {
        for (band, scaling) in residual_scaling.iter_mut().enumerate() {
            let stationary = self.render_stationarity.is_band_stationary(band)
                && (filter_has_had_time_to_converge || self.use_render_stationarity_at_init);
            *scaling = if stationary { 0.0 } else { 1.0 };
        }
    }

    pub(crate) fn is_block_stationary(&self) -> bool {
        self.render_stationarity.is_block_stationary()
    }

    fn update_render_noise_estimator(&mut self, render: &RenderBuffer, external_delay_seen: bool) {
        let spectra = &render.spectra;
        let Some(previous) = self.render_spectrum_write_prev else {
            self.render_spectrum_write_prev = Some(spectra.write);
            self.render_block_write_prev = render.blocks.write;
            return;
        };
        let current = spectra.write;
        if !self.non_zero_render_seen && !external_delay_seen {
            self.non_zero_render_seen = !self.is_render_too_low(render);
        }
        if self.non_zero_render_seen {
            let mut idx = previous;
            while idx != current {
                self.render_stationarity
                    .update_noise_estimator(&spectra.buffer[idx]);
                idx = spectra.dec(idx);
            }
        }
        self.render_spectrum_write_prev = Some(current);
    }

    fn is_render_too_low(&mut self, render: &RenderBuffer) -> bool {
        let blocks = &render.blocks;
        let current = blocks.write;
        let mut too_low = false;
        if current == self.render_block_write_prev {
            too_low = true;
        } else {
            let mut idx = self.render_block_write_prev;
            while idx != current {
                let block = &blocks.buffer[idx];
                let mut max_abs_over_channels = 0.0_f32;
                for ch in 0..block.channels() {
                    let view: &[f32; BLOCK_LEN] = block.view(0, ch);
                    let (min, max) = view.iter().fold((view[0], view[0]), |(min, max), value| {
                        (min.min(*value), max.max(*value))
                    });
                    max_abs_over_channels = max_abs_over_channels.max(min.abs().max(max.abs()));
                }
                if max_abs_over_channels < 10.0 {
                    too_low = true;
                    break;
                }
                idx = blocks.inc(idx);
            }
        }
        self.render_block_write_prev = current;
        too_low
    }
}
