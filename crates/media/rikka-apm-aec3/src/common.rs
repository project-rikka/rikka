pub(crate) const BLOCK_LEN: usize = 64;
pub(crate) const BLOCK_LEN_LOG2: usize = 6;
pub(crate) const FFT_LEN_BY_2: usize = 64;
pub(crate) const FFT_LEN_BY_2_LOG2: usize = 6;
pub(crate) const FFT_LEN: usize = 128;
pub(crate) const SPECTRUM_LEN: usize = 65;
pub(crate) const BLOCKS_PER_SECOND: usize = 250;
pub(crate) const SUB_FRAME_LEN: usize = 80;
pub(crate) const MATCHED_FILTER_WINDOW_SUB_BLOCKS: usize = 32;
pub(crate) const MATCHED_FILTER_ALIGNMENT_SHIFT_SUB_BLOCKS: usize =
    MATCHED_FILTER_WINDOW_SUB_BLOCKS * 3 / 4;
pub(crate) const RENDER_TRANSFER_QUEUE_FRAMES: usize = 100;

pub(crate) type Spectrum = [f32; SPECTRUM_LEN];

pub(crate) const fn time_domain_len(blocks: usize) -> usize {
    blocks * FFT_LEN_BY_2
}

pub(crate) const fn downsampled_buffer_len(down_sampling_factor: usize, filters: usize) -> usize {
    BLOCK_LEN / down_sampling_factor
        * (MATCHED_FILTER_ALIGNMENT_SHIFT_SUB_BLOCKS * filters
            + MATCHED_FILTER_WINDOW_SUB_BLOCKS
            + 1)
}

pub(crate) const fn render_delay_buffer_len(
    down_sampling_factor: usize,
    filters: usize,
    filter_blocks: usize,
) -> usize {
    downsampled_buffer_len(down_sampling_factor, filters) / (BLOCK_LEN / down_sampling_factor)
        + filter_blocks
        + 1
}

pub(crate) fn fast_approx_log2(value: f32) -> f32 {
    let mut out = value.to_bits() as f32;
    out *= 1.192_092_9e-7;
    out -= 126.942_695;
    out
}

pub(crate) fn log2_to_db(log2: f32) -> f32 {
    (3.010_299_956_639_812_f64 * f64::from(log2)) as f32
}

pub(crate) fn safe_clamp(value: f32, low: f32, high: f32) -> f32 {
    if value <= low {
        low
    } else if value >= high {
        high
    } else {
        value
    }
}

pub(crate) fn sum_of_squares(values: &[f32]) -> f32 {
    values.iter().fold(0.0, |sum, value| sum + value * value)
}

pub(crate) fn sum(values: &[f32]) -> f32 {
    values.iter().fold(0.0, |sum, value| sum + value)
}
