const MIN_VARIANCE: f64 = 256.0;

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub(super) struct Detail(f64);

impl Detail {
    pub(super) fn measure(rgba: &[u8]) -> Detail {
        let samples = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|pixel| &pixel[..3])
            .map(|&sample| f64::from(sample));
        let (count, sum, squares) = samples.fold((0.0, 0.0, 0.0), |(count, sum, squares), x| {
            (count + 1.0, sum + x, squares + x * x)
        });
        if count == 0.0 {
            return Detail(0.0);
        }
        let mean = sum / count;
        Detail((squares / count - mean * mean).max(0.0))
    }

    pub(super) fn is_sufficient(self) -> bool {
        self.0 > MIN_VARIANCE
    }
}
