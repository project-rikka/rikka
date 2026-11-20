mod algorithm;
mod convolution;
mod cursor;
mod interpolator;
mod kernel;
mod lagrange;
mod phase_bank;
mod phases;
mod planes;
mod polynomial_degree;
mod polynomial_kernel;
mod quality;
mod resampler;
mod sinc_kernel;
mod sinc_params;
mod sinc_table;
mod weights;
mod window;

pub use quality::ResampleQuality;

pub use resampler::Resampler;

fn output_frames(frames: u64, in_rate: u32, out_rate: u32) -> u64 {
    (frames * u64::from(out_rate.max(1))).div_ceil(u64::from(in_rate.max(1)))
}

#[must_use]
pub fn resample(
    samples: &[f32],
    channels: u16,
    in_rate: u32,
    out_rate: u32,
    quality: ResampleQuality,
) -> Vec<f32> {
    if in_rate == out_rate {
        return samples.to_vec();
    }
    let channels = channels.max(1) as usize;
    let frames = (samples.len() / channels) as u64;
    let capacity = output_frames(frames, in_rate, out_rate) as usize * channels;

    let mut resampler = Resampler::with_quality(quality, in_rate, out_rate, channels as u16);
    let mut output = Vec::with_capacity(capacity);
    resampler.process(samples, &mut output);
    resampler.flush(&mut output);
    output
}
