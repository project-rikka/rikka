#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub lowest_bps: u64,
    pub start_bps: u64,
    pub highest_bps: u64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Rates {
    pub(crate) lowest: f64,
    pub(crate) start: f64,
    pub(crate) highest: f64,
}

impl Limits {
    pub(crate) fn rates(self) -> Rates {
        let lowest = self.lowest_bps as f64;
        let highest = (self.highest_bps as f64).max(lowest);
        Rates {
            lowest,
            start: (self.start_bps as f64).clamp(lowest, highest),
            highest,
        }
    }
}
