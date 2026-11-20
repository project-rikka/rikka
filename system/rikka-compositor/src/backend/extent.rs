const WIDEST: u32 = 7680;
const TALLEST: u32 = 4320;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extent {
    pub width: u32,
    pub height: u32,
}

impl Extent {
    #[must_use]
    pub fn clamped(self) -> Extent {
        Extent {
            width: self.width.clamp(1, WIDEST),
            height: self.height.clamp(1, TALLEST),
        }
    }
}
