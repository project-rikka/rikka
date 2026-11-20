use crate::mapping::Encoding;
use crate::primaries::Primaries;
use crate::transfer::{PQ_PEAK, TransferFunction};

const SDR_ANCHOR: f64 = 203.0;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum BlendSpace {
    #[default]
    Sdr,
    Hdr(HdrSpace),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HdrSpace {
    pub reference: f64,
    pub peak: f64,
}

impl BlendSpace {
    pub fn primaries(&self) -> Primaries {
        match self {
            Self::Sdr => Primaries::SRGB,
            Self::Hdr(_) => Primaries::BT2020,
        }
    }

    pub fn headroom(&self) -> f64 {
        match self {
            Self::Sdr => 1.0,
            Self::Hdr(hdr) => (hdr.peak / hdr.reference).max(1.0),
        }
    }

    pub fn anchor(&self) -> f64 {
        match self {
            Self::Sdr => SDR_ANCHOR,
            Self::Hdr(hdr) => hdr.reference,
        }
    }

    pub fn encoding(&self) -> Encoding {
        match self {
            Self::Sdr => Encoding::Gamma22,
            Self::Hdr(_) => Encoding::Linear,
        }
    }

    pub fn is_hdr(&self) -> bool {
        matches!(self, Self::Hdr(_))
    }
}

impl HdrSpace {
    pub fn signal_scale(&self) -> f64 {
        self.reference / PQ_PEAK
    }

    pub fn encode(&self, linear: [f64; 3]) -> [f64; 3] {
        linear.map(|channel| TransferFunction::Pq.encode(channel.max(0.0) * self.signal_scale()))
    }
}
