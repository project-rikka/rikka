pub(crate) struct Cursor {
    index: usize,
    numerator: u64,
    step: u64,
    denominator: u64,
}

impl Cursor {
    pub(crate) fn new(in_rate: u32, out_rate: u32) -> Cursor {
        let in_rate = u64::from(in_rate);
        let out_rate = u64::from(out_rate);
        let divisor = gcd(in_rate, out_rate);
        Cursor {
            index: 0,
            numerator: 0,
            step: in_rate / divisor,
            denominator: out_rate / divisor,
        }
    }

    pub(crate) fn index(&self) -> usize {
        self.index
    }

    pub(crate) fn ratio(&self) -> f64 {
        self.denominator as f64 / self.step as f64
    }

    pub(crate) fn phases(&self) -> usize {
        self.denominator as usize
    }

    pub(crate) fn phase(&self) -> usize {
        self.numerator as usize
    }

    pub(crate) fn frac(&self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }

    pub(crate) fn advance(&mut self) {
        self.numerator += self.step;
        self.index += (self.numerator / self.denominator) as usize;
        self.numerator %= self.denominator;
    }

    pub(crate) fn restart(&mut self, index: usize) {
        self.index = index;
        self.numerator = 0;
    }

    pub(crate) fn rebase(&mut self, frames: usize) {
        self.index -= frames;
    }
}

pub(crate) fn gcd(a: u64, b: u64) -> u64 {
    let mut a = a;
    let mut b = b;
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}
