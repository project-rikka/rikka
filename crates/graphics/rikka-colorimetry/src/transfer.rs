pub const PQ_PEAK: f64 = 10_000.0;

const PQ_M1: f64 = 2610.0 / 16384.0;
const PQ_M2: f64 = 2523.0 / 4096.0 * 128.0;
const PQ_C1: f64 = 3424.0 / 4096.0;
const PQ_C2: f64 = 2413.0 / 4096.0 * 32.0;
const PQ_C3: f64 = 2392.0 / 4096.0 * 32.0;

const HLG_A: f64 = 0.178_832_77;
const HLG_B: f64 = 1.0 - 4.0 * HLG_A;
const HLG_C: f64 = 0.559_910_73;

const SRGB_KNEE: f64 = 0.040_45;
const SRGB_LINEAR_KNEE: f64 = 0.003_130_8;
const SRGB_SLOPE: f64 = 12.92;
const SRGB_OFFSET: f64 = 0.055;
const SRGB_EXPONENT: f64 = 2.4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransferFunction {
    Power(f64),
    CompoundPower24,
    Linear,
    Pq,
    Hlg,
}

impl TransferFunction {
    pub const GAMMA22: Self = Self::Power(2.2);
    pub const GAMMA28: Self = Self::Power(2.8);
    pub const BT1886: Self = Self::Power(2.4);

    pub fn decode(self, encoded: f64) -> f64 {
        match self {
            Self::Power(exponent) => signed(encoded, |value| value.powf(exponent)),
            Self::CompoundPower24 => signed(encoded, srgb_decode),
            Self::Linear => encoded,
            Self::Pq => pq_decode(encoded),
            Self::Hlg => hlg_decode(encoded),
        }
    }

    pub fn encode(self, linear: f64) -> f64 {
        match self {
            Self::Power(exponent) => signed(linear, |value| value.powf(exponent.recip())),
            Self::CompoundPower24 => signed(linear, srgb_encode),
            Self::Linear => linear,
            Self::Pq => pq_encode(linear),
            Self::Hlg => hlg_encode(linear),
        }
    }

    pub fn is_absolute(self) -> bool {
        self == Self::Pq
    }
}

fn signed(value: f64, curve: impl Fn(f64) -> f64) -> f64 {
    curve(value.abs()).copysign(value)
}

fn srgb_decode(encoded: f64) -> f64 {
    if encoded <= SRGB_KNEE {
        encoded / SRGB_SLOPE
    } else {
        ((encoded + SRGB_OFFSET) / (1.0 + SRGB_OFFSET)).powf(SRGB_EXPONENT)
    }
}

fn srgb_encode(linear: f64) -> f64 {
    if linear <= SRGB_LINEAR_KNEE {
        linear * SRGB_SLOPE
    } else {
        (1.0 + SRGB_OFFSET) * linear.powf(SRGB_EXPONENT.recip()) - SRGB_OFFSET
    }
}

fn pq_decode(encoded: f64) -> f64 {
    let raised = encoded.clamp(0.0, 1.0).powf(PQ_M2.recip());
    ((raised - PQ_C1).max(0.0) / (PQ_C2 - PQ_C3 * raised)).powf(PQ_M1.recip())
}

fn pq_encode(linear: f64) -> f64 {
    let raised = linear.clamp(0.0, 1.0).powf(PQ_M1);
    ((PQ_C1 + PQ_C2 * raised) / (1.0 + PQ_C3 * raised)).powf(PQ_M2)
}

fn hlg_decode(encoded: f64) -> f64 {
    let encoded = encoded.clamp(0.0, 1.0);
    if encoded <= 0.5 {
        encoded * encoded / 3.0
    } else {
        (((encoded - HLG_C) / HLG_A).exp() + HLG_B) / 12.0
    }
}

fn hlg_encode(linear: f64) -> f64 {
    let linear = linear.clamp(0.0, 1.0);
    if linear <= 1.0 / 12.0 {
        (3.0 * linear).sqrt()
    } else {
        HLG_A * (12.0 * linear - HLG_B).ln() + HLG_C
    }
}
