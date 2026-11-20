use crate::common::{FFT_LEN, FFT_LEN_BY_2, SPECTRUM_LEN, Spectrum, time_domain_len};
use crate::fft::{self, FftData};
use crate::render::RenderBuffer;

#[derive(Clone, Debug)]
pub(crate) struct AdaptiveFirFilter {
    render_channels: usize,
    max_size_partitions: usize,
    size_change_duration_blocks: i32,
    one_by_size_change_duration_blocks: f32,
    current_size_partitions: usize,
    target_size_partitions: usize,
    old_target_size_partitions: usize,
    size_change_counter: i32,
    h: Vec<Vec<FftData>>,
    partition_to_constrain: usize,
}

impl AdaptiveFirFilter {
    pub(crate) fn new(
        max_size_partitions: usize,
        initial_size_partitions: usize,
        size_change_duration_blocks: usize,
        render_channels: usize,
    ) -> AdaptiveFirFilter {
        let size_change_duration_blocks = size_change_duration_blocks as i32;
        let mut filter = AdaptiveFirFilter {
            render_channels,
            max_size_partitions,
            size_change_duration_blocks,
            one_by_size_change_duration_blocks: 1.0 / size_change_duration_blocks as f32,
            current_size_partitions: initial_size_partitions,
            target_size_partitions: initial_size_partitions,
            old_target_size_partitions: initial_size_partitions,
            size_change_counter: 0,
            h: vec![vec![FftData::default(); render_channels]; max_size_partitions],
            partition_to_constrain: 0,
        };
        filter.set_size_partitions(initial_size_partitions, true);
        filter
    }

    pub(crate) fn size_partitions(&self) -> usize {
        self.current_size_partitions
    }

    pub(crate) fn filter_partitions(&self) -> &[Vec<FftData>] {
        &self.h
    }

    pub(crate) fn handle_echo_path_change(&mut self) {
        self.zero_filter(self.current_size_partitions, self.max_size_partitions);
    }

    pub(crate) fn set_size_partitions(&mut self, size: usize, immediate_effect: bool) {
        self.target_size_partitions = self.max_size_partitions.min(size);
        if immediate_effect {
            let old_size_partitions = self.current_size_partitions;
            self.current_size_partitions = self.target_size_partitions;
            self.old_target_size_partitions = self.target_size_partitions;
            self.zero_filter(old_size_partitions, self.current_size_partitions);
            self.partition_to_constrain = self
                .partition_to_constrain
                .min(self.current_size_partitions - 1);
            self.size_change_counter = 0;
        } else {
            self.size_change_counter = self.size_change_duration_blocks;
        }
    }

    pub(crate) fn filter(&self, render: &RenderBuffer, s: &mut FftData) {
        s.clear();
        let buffer = render.fft_buffer();
        let mut index = render.position();
        for partition in &self.h[..self.current_size_partitions] {
            for (x, h) in buffer[index].iter().zip(partition) {
                for k in 0..SPECTRUM_LEN {
                    s.re[k] += x.re[k] * h.re[k] - x.im[k] * h.im[k];
                    s.im[k] += x.re[k] * h.im[k] + x.im[k] * h.re[k];
                }
            }
            index = if index < buffer.len() - 1 {
                index + 1
            } else {
                0
            };
        }
    }

    pub(crate) fn adapt(&mut self, render: &RenderBuffer, g: &FftData) {
        self.adapt_and_update_size(render, g);
        self.constrain();
    }

    pub(crate) fn adapt_with_impulse_response(
        &mut self,
        render: &RenderBuffer,
        g: &FftData,
        impulse_response: &mut Vec<f32>,
    ) {
        self.adapt_and_update_size(render, g);
        self.constrain_and_update_impulse_response(impulse_response);
    }

    pub(crate) fn compute_frequency_response(&self, h2: &mut Vec<Spectrum>) {
        h2.resize(self.current_size_partitions, [0.0; SPECTRUM_LEN]);
        for (h2, partition) in h2.iter_mut().zip(&self.h) {
            *h2 = [0.0; SPECTRUM_LEN];
            for h in partition {
                for k in 0..SPECTRUM_LEN {
                    let power = h.re[k] * h.re[k] + h.im[k] * h.im[k];
                    h2[k] = h2[k].max(power);
                }
            }
        }
    }

    pub(crate) fn scale_filter(&mut self, factor: f32) {
        for partition in &mut self.h {
            for h in partition {
                for value in h.re.iter_mut().chain(h.im.iter_mut()) {
                    *value *= factor;
                }
            }
        }
    }

    pub(crate) fn set_filter(&mut self, partitions: usize, h: &[Vec<FftData>]) {
        let count = self.current_size_partitions.min(partitions);
        for (own, other) in self.h[..count].iter_mut().zip(h) {
            for (own, other) in own[..self.render_channels].iter_mut().zip(other) {
                own.re = other.re;
                own.im = other.im;
            }
        }
    }

    fn zero_filter(&mut self, old_size: usize, new_size: usize) {
        for partition in self.h.iter_mut().take(new_size).skip(old_size) {
            for h in partition {
                h.clear();
            }
        }
    }

    fn update_size(&mut self) {
        let old_size_partitions = self.current_size_partitions;
        if self.size_change_counter > 0 {
            self.size_change_counter -= 1;
            let change_factor =
                self.size_change_counter as f32 * self.one_by_size_change_duration_blocks;
            let from = self.old_target_size_partitions as f32;
            let to = self.target_size_partitions as f32;
            self.current_size_partitions =
                (from * change_factor + to * (1.0 - change_factor)) as usize;
            self.partition_to_constrain = self
                .partition_to_constrain
                .min(self.current_size_partitions - 1);
        } else {
            self.current_size_partitions = self.target_size_partitions;
            self.old_target_size_partitions = self.target_size_partitions;
        }
        self.zero_filter(old_size_partitions, self.current_size_partitions);
    }

    fn adapt_and_update_size(&mut self, render: &RenderBuffer, g: &FftData) {
        self.update_size();
        let buffer = render.fft_buffer();
        let mut index = render.position();
        for partition in &mut self.h[..self.current_size_partitions] {
            for (x, h) in buffer[index].iter().zip(partition.iter_mut()) {
                for k in 0..SPECTRUM_LEN {
                    h.re[k] += x.re[k] * g.re[k] + x.im[k] * g.im[k];
                    h.im[k] += x.re[k] * g.im[k] - x.im[k] * g.re[k];
                }
            }
            index = if index < buffer.len() - 1 {
                index + 1
            } else {
                0
            };
        }
    }

    fn constrain_partition(h: &mut FftData) -> [f32; FFT_LEN] {
        const SCALE: f32 = 1.0 / FFT_LEN_BY_2 as f32;
        let mut time = [0.0; FFT_LEN];
        fft::ifft(h, &mut time);
        for value in &mut time[..FFT_LEN_BY_2] {
            *value *= SCALE;
        }
        time[FFT_LEN_BY_2..].fill(0.0);
        let constrained = time;
        fft::fft(&mut time, h);
        constrained
    }

    fn constrain_and_update_impulse_response(&mut self, impulse_response: &mut Vec<f32>) {
        impulse_response.resize(time_domain_len(self.current_size_partitions), 0.0);
        let start = self.partition_to_constrain * FFT_LEN_BY_2;
        let region = &mut impulse_response[start..start + FFT_LEN_BY_2];
        region.fill(0.0);
        for (channel, h) in self.h[self.partition_to_constrain].iter_mut().enumerate() {
            let time = Self::constrain_partition(h);
            if channel == 0 {
                region.copy_from_slice(&time[..FFT_LEN_BY_2]);
            } else {
                for (out, value) in region.iter_mut().zip(&time[..FFT_LEN_BY_2]) {
                    if out.abs() < value.abs() {
                        *out = *value;
                    }
                }
            }
        }
        self.advance_partition_to_constrain();
    }

    fn constrain(&mut self) {
        for h in &mut self.h[self.partition_to_constrain] {
            Self::constrain_partition(h);
        }
        self.advance_partition_to_constrain();
    }

    fn advance_partition_to_constrain(&mut self) {
        self.partition_to_constrain =
            if self.partition_to_constrain < self.current_size_partitions - 1 {
                self.partition_to_constrain + 1
            } else {
                0
            };
    }
}
