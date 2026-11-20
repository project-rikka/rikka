use rikka_resample::Resampler;

use crate::config::RESAMPLE_QUALITY;
use crate::mixer::Mixer;
use crate::pan::Pan;

const BLOCK_FRAMES: usize = 256;
const UNITY: std::ops::Range<f32> = 1.0..1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Layout {
    pub(super) rate: u32,
    pub(super) channels: u16,
}

pub(super) struct ToDevice {
    resampler: Option<Resampler>,
    pan: Option<Pan>,
    source_channels: usize,
    device_channels: usize,
    block: Vec<f32>,
    pending: Vec<f32>,
    taken: usize,
    scratch: Vec<f32>,
}

impl ToDevice {
    pub(super) fn new(source: Layout, device: Layout, device_frames: usize) -> ToDevice {
        let source_channels = usize::from(source.channels);
        let device_channels = usize::from(device.channels);
        let resampler = (source.rate != device.rate).then(|| {
            Resampler::with_quality(RESAMPLE_QUALITY, source.rate, device.rate, source.channels)
        });
        let stretch = device.rate.div_ceil(source.rate.max(1)) as usize + 1;
        ToDevice {
            resampler,
            pan: (source.channels != device.channels)
                .then(|| Pan::new(0.0, source.channels, device.channels)),
            source_channels,
            device_channels,
            block: vec![0.0; BLOCK_FRAMES * source_channels],
            pending: Vec::with_capacity((device_frames + BLOCK_FRAMES * stretch) * source_channels),
            taken: 0,
            scratch: vec![0.0; device_frames * device_channels],
        }
    }

    pub(super) fn render(&mut self, mixer: &mut Mixer, out: &mut [f32]) {
        let frames = out.len() / self.device_channels;
        match self.resampler.as_mut() {
            None => match self.pan {
                None => mixer.render(out),
                Some(pan) => {
                    for chunk in out.chunks_mut(BLOCK_FRAMES * self.device_channels) {
                        let block = &mut self.block
                            [..chunk.len() / self.device_channels * self.source_channels];
                        mixer.render(block);
                        chunk.fill(0.0);
                        pan.mix(block, UNITY, chunk);
                    }
                }
            },
            Some(resampler) => {
                let wanted = frames * self.source_channels;
                self.pending.drain(..self.taken);
                self.taken = 0;
                while self.pending.len() < wanted {
                    mixer.render(&mut self.block);
                    resampler.process(&self.block, &mut self.pending);
                }
                let source = &self.pending[..wanted];
                match self.pan {
                    None => out.copy_from_slice(source),
                    Some(pan) => {
                        out.fill(0.0);
                        pan.mix(source, UNITY, out);
                    }
                }
                self.taken = wanted;
            }
        }
    }

    pub(super) fn render_unaligned(&mut self, mixer: &mut Mixer, out: &mut [u8]) {
        let samples = out.len() / size_of::<f32>();
        let mut scratch = std::mem::take(&mut self.scratch);
        if scratch.len() < samples {
            scratch.resize(samples, 0.0);
        }
        self.render(mixer, &mut scratch[..samples]);
        for (bytes, sample) in out.as_chunks_mut::<4>().0.iter_mut().zip(&scratch) {
            *bytes = sample.to_ne_bytes();
        }
        self.scratch = scratch;
    }
}

pub(super) struct FromDevice {
    resampler: Option<Resampler>,
    pan: Option<Pan>,
    target_channels: usize,
    mapped: Vec<f32>,
    resampled: Vec<f32>,
}

impl FromDevice {
    pub(super) fn new(device: Layout, target: Layout) -> FromDevice {
        FromDevice {
            resampler: (device.rate != target.rate).then(|| {
                Resampler::with_quality(RESAMPLE_QUALITY, device.rate, target.rate, target.channels)
            }),
            pan: (device.channels != target.channels)
                .then(|| Pan::new(0.0, device.channels, target.channels)),
            target_channels: usize::from(target.channels),
            mapped: Vec::new(),
            resampled: Vec::new(),
        }
    }

    pub(super) fn convert<'a>(&'a mut self, device: &'a [f32], frames: usize) -> &'a [f32] {
        let mapped: &[f32] = match self.pan {
            None => device,
            Some(pan) => {
                self.mapped.clear();
                self.mapped.resize(frames * self.target_channels, 0.0);
                pan.mix(device, UNITY, &mut self.mapped);
                &self.mapped
            }
        };
        match self.resampler.as_mut() {
            None => mapped,
            Some(resampler) => {
                self.resampled.clear();
                resampler.process(mapped, &mut self.resampled);
                &self.resampled
            }
        }
    }
}
