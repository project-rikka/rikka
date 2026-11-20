#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Chromaticity {
    pub x: f64,
    pub y: f64,
}

impl Chromaticity {
    pub const D65: Self = Self::new(0.3127, 0.3290);
    pub const D50: Self = Self::new(0.3457, 0.3585);
    pub const ILLUMINANT_C: Self = Self::new(0.310, 0.316);
    pub const DCI_WHITE: Self = Self::new(0.314, 0.351);
    pub const EQUAL_ENERGY: Self = Self::new(1.0 / 3.0, 1.0 / 3.0);

    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn to_xyz(self) -> [f64; 3] {
        [self.x / self.y, 1.0, (1.0 - self.x - self.y) / self.y]
    }

    pub(crate) fn column(self) -> [f64; 3] {
        [self.x, self.y, 1.0 - self.x - self.y]
    }
}
