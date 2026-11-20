#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum WindowFunction {
    BlackmanHarris,
    BlackmanHarris2,
}

const BLACKMAN_HARRIS: [f64; 4] = [0.35875, 0.48829, 0.14128, 0.01168];

impl WindowFunction {
    pub(crate) fn evaluate(self, u: f64, half: f64) -> f64 {
        if u.abs() > half {
            return 0.0;
        }
        let a = BLACKMAN_HARRIS;
        let cos1 = (std::f64::consts::PI * u / half).cos();
        let cos2 = 2.0 * cos1 * cos1 - 1.0;
        let cos3 = (2.0 * cos2 - 1.0) * cos1;
        let value = a[0] + a[1] * cos1 + a[2] * cos2 + a[3] * cos3;
        match self {
            WindowFunction::BlackmanHarris => value,
            WindowFunction::BlackmanHarris2 => value * value,
        }
    }

    pub(crate) fn cutoff(self, taps: usize) -> f64 {
        let (k1, k2, k3) = match self {
            WindowFunction::BlackmanHarris => (
                8.041_443_677_716_476,
                55.950_677_934_338_7,
                898.028_798_538_421_3,
            ),
            WindowFunction::BlackmanHarris2 => (
                13.745_202_940_783_823,
                121.735_325_863_749_34,
                5_964.163_279_612_051,
            ),
        };
        let n = taps as f64;
        1.0 / (k1 / n + k2 / (n * n) + k3 / (n * n * n) + 1.0)
    }
}
