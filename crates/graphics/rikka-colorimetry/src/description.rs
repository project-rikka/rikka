use crate::primaries::Primaries;
use crate::transfer::{PQ_PEAK, TransferFunction};

const SRGB_LUMINANCE: Luminance = Luminance {
    min: 0.2,
    max: 80.0,
    reference: 80.0,
};

const PQ_LUMINANCE: Luminance = Luminance {
    min: 0.005,
    max: PQ_PEAK,
    reference: 203.0,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Luminance {
    pub min: f64,
    pub max: f64,
    pub reference: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mastering {
    pub primaries: Primaries,
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Intent {
    #[default]
    Perceptual,
    Relative,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Description {
    pub primaries: Primaries,
    pub transfer: TransferFunction,
    pub luminance: Luminance,
    pub mastering: Option<Mastering>,
    pub max_cll: Option<f64>,
    pub max_fall: Option<f64>,
}

impl Description {
    pub const SRGB: Self = Self {
        primaries: Primaries::SRGB,
        transfer: TransferFunction::GAMMA22,
        luminance: SRGB_LUMINANCE,
        mastering: None,
        max_cll: None,
        max_fall: None,
    };

    pub const BT2100_PQ: Self = Self {
        primaries: Primaries::BT2020,
        transfer: TransferFunction::Pq,
        luminance: PQ_LUMINANCE,
        mastering: None,
        max_cll: None,
        max_fall: None,
    };

    pub fn peak(&self) -> f64 {
        self.max_cll
            .or(self.mastering.map(|mastering| mastering.max))
            .unwrap_or(self.luminance.max)
    }

    pub fn relative_peak(&self) -> f64 {
        self.peak() / self.luminance.reference
    }
}
