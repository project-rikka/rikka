const SEED: u64 = 0x9E37_79B9_7F4A_7C15;
const UNIT: f64 = 1.0 / (1_u64 << 53) as f64;

#[derive(Debug, Clone)]
pub(crate) struct Tpdf(u64);

impl Tpdf {
    pub(crate) fn new() -> Tpdf {
        Tpdf(SEED)
    }

    pub(crate) fn next(&mut self) -> f64 {
        self.uniform() - self.uniform()
    }

    fn uniform(&mut self) -> f64 {
        let mut state = self.0;
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        self.0 = state;
        (state >> 11) as f64 * UNIT
    }
}
