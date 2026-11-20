use crate::chromaticity::Chromaticity;
use crate::matrix::Matrix3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Primaries {
    pub red: Chromaticity,
    pub green: Chromaticity,
    pub blue: Chromaticity,
    pub white: Chromaticity,
}

const fn xy(x: f64, y: f64) -> Chromaticity {
    Chromaticity::new(x, y)
}

impl Primaries {
    pub const SRGB: Self = Self::new(
        xy(0.64, 0.33),
        xy(0.30, 0.60),
        xy(0.15, 0.06),
        Chromaticity::D65,
    );
    pub const PAL_M: Self = Self::new(
        xy(0.67, 0.33),
        xy(0.21, 0.71),
        xy(0.14, 0.08),
        Chromaticity::ILLUMINANT_C,
    );
    pub const PAL: Self = Self::new(
        xy(0.64, 0.33),
        xy(0.29, 0.60),
        xy(0.15, 0.06),
        Chromaticity::D65,
    );
    pub const NTSC: Self = Self::new(
        xy(0.630, 0.340),
        xy(0.310, 0.595),
        xy(0.155, 0.070),
        Chromaticity::D65,
    );
    pub const GENERIC_FILM: Self = Self::new(
        xy(0.681, 0.319),
        xy(0.243, 0.692),
        xy(0.145, 0.049),
        Chromaticity::ILLUMINANT_C,
    );
    pub const BT2020: Self = Self::new(
        xy(0.708, 0.292),
        xy(0.170, 0.797),
        xy(0.131, 0.046),
        Chromaticity::D65,
    );
    pub const CIE1931_XYZ: Self = Self::new(
        xy(1.0, 0.0),
        xy(0.0, 1.0),
        xy(0.0, 0.0),
        Chromaticity::EQUAL_ENERGY,
    );
    pub const DCI_P3: Self = Self::new(
        xy(0.680, 0.320),
        xy(0.265, 0.690),
        xy(0.150, 0.060),
        Chromaticity::DCI_WHITE,
    );
    pub const DISPLAY_P3: Self = Self::new(
        xy(0.680, 0.320),
        xy(0.265, 0.690),
        xy(0.150, 0.060),
        Chromaticity::D65,
    );
    pub const ADOBE_RGB: Self = Self::new(
        xy(0.64, 0.33),
        xy(0.21, 0.71),
        xy(0.15, 0.06),
        Chromaticity::D65,
    );

    pub const fn new(
        red: Chromaticity,
        green: Chromaticity,
        blue: Chromaticity,
        white: Chromaticity,
    ) -> Self {
        Self {
            red,
            green,
            blue,
            white,
        }
    }

    pub fn to_xyz(&self) -> Option<Matrix3> {
        let corners =
            Matrix3::from_columns([self.red.column(), self.green.column(), self.blue.column()]);
        let weights = corners.inverse()?.apply(self.white.to_xyz());
        Some(corners * Matrix3::diagonal(weights))
    }

    pub fn is_valid(&self) -> bool {
        let inside = |point: Chromaticity| {
            point.x.is_finite() && point.y.is_finite() && point.x >= 0.0 && point.y >= 0.0
        };
        [self.red, self.green, self.blue, self.white]
            .into_iter()
            .all(inside)
            && self.white.y > 0.0
            && self.to_xyz().is_some()
    }

    pub fn luminance_coefficients(&self) -> Option<[f64; 3]> {
        Some(self.to_xyz()?.rows()[1])
    }
}
