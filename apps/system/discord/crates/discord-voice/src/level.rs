pub(crate) const SILENT_DB: f32 = -100.0;

pub(crate) fn rms_db(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return SILENT_DB;
    }
    let energy = samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32;
    if energy <= 0.0 {
        return SILENT_DB;
    }
    (10.0 * energy.log10()).max(SILENT_DB)
}
