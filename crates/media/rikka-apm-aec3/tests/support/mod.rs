pub(crate) mod banded;
pub(crate) mod fullband;

pub(crate) struct Lcg(u32);

impl Lcg {
    pub(crate) fn new(seed: u32) -> Lcg {
        Lcg(seed)
    }

    pub(crate) fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 * (1.0 / 8_388_608.0) - 1.0
    }
}

pub(crate) fn energy(samples: &[f32]) -> f64 {
    samples.iter().map(|v| f64::from(*v) * f64::from(*v)).sum()
}

pub(crate) fn db(ratio: f64) -> f64 {
    10.0 * ratio.log10()
}
