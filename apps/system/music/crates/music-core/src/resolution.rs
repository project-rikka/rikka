const CD_BITS: u32 = 16;
const CD_RATE: u32 = 48_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Resolution {
    pub bits: u32,
    pub rate: u32,
}

impl Resolution {
    pub const CD: Resolution = Resolution {
        bits: CD_BITS,
        rate: 44_100,
    };

    #[must_use]
    pub fn is_high(self) -> bool {
        self.bits > CD_BITS || self.rate > CD_RATE
    }
}
