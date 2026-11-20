pub(super) fn spread(bands: &[f32], into: &mut [f32]) {
    let count = bands.len();
    let band_count = into.len();
    for (index, slot) in into.iter_mut().enumerate() {
        let from = (index * count / band_count).min(count - 1);
        let to = ((index + 1) * count / band_count).clamp(from + 1, count);
        *slot = bands[from..to].iter().copied().fold(0.0, f32::max);
    }
}
