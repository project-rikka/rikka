use crate::adaptation::bradford;
use crate::description::{Description, Intent};
use crate::eetf::Eetf;
use crate::matrix::Matrix3;
use crate::space::BlendSpace;
use crate::transfer::{PQ_PEAK, TransferFunction};

const HLG_REFERENCE_PEAK: f64 = 1000.0;
const HLG_GAMMA: f64 = 1.2;
const HLG_GAMMA_SLOPE: f64 = 0.42;
const IDENTITY_TOLERANCE: f64 = 1.0e-6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Gamma22,
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ootf {
    pub gamma: f64,
    pub luminance: [f64; 3],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mapping {
    pub decode: TransferFunction,
    pub scale: f64,
    pub ootf: Option<Ootf>,
    pub matrix: Matrix3,
    pub tone: Option<Eetf>,
    pub encoding: Encoding,
}

impl Mapping {
    pub fn new(source: &Description, intent: Intent, target: &BlendSpace) -> Option<Self> {
        let into_xyz = source.primaries.to_xyz()?;
        let destination = target.primaries();
        let from_xyz = destination.to_xyz()?.inverse()?;
        let adapt = bradford(source.primaries.white, destination.white);
        let luminance = source.luminance;
        let (scale, ootf) = match source.transfer {
            TransferFunction::Pq => (PQ_PEAK / luminance.reference, None),
            TransferFunction::Hlg => (
                luminance.max / luminance.reference,
                Some(Ootf {
                    gamma: HLG_GAMMA
                        + HLG_GAMMA_SLOPE * (luminance.max / HLG_REFERENCE_PEAK).log10(),
                    luminance: source.primaries.luminance_coefficients()?,
                }),
            ),
            _ => (luminance.max / luminance.reference, None),
        };
        let tone = match intent {
            Intent::Perceptual => {
                Eetf::new(target.anchor(), source.relative_peak(), target.headroom())
            }
            Intent::Relative => None,
        };
        Some(Self {
            decode: source.transfer,
            scale,
            ootf,
            matrix: from_xyz * adapt * into_xyz,
            tone,
            encoding: target.encoding(),
        })
    }

    pub fn apply(&self, encoded: [f64; 3]) -> [f64; 3] {
        let mut linear = encoded.map(|channel| self.decode.decode(channel));
        if let Some(ootf) = self.ootf {
            let luminance: f64 = linear
                .iter()
                .zip(ootf.luminance)
                .map(|(channel, weight)| channel * weight)
                .sum();
            let gain = match luminance > 0.0 {
                true => luminance.powf(ootf.gamma - 1.0),
                false => 0.0,
            };
            linear = linear.map(|channel| channel * gain);
        }
        let mut color = self
            .matrix
            .apply(linear.map(|channel| channel * self.scale));
        if let Some(tone) = self.tone {
            let peak = color[0].max(color[1]).max(color[2]);
            if peak > 0.0 {
                let gain = tone.map(peak) / peak;
                color = color.map(|channel| channel * gain);
            }
        }
        match self.encoding {
            Encoding::Gamma22 => {
                color.map(|channel| TransferFunction::GAMMA22.encode(channel.clamp(0.0, 1.0)))
            }
            Encoding::Linear => color.map(|channel| channel.max(0.0)),
        }
    }

    pub fn is_identity(&self) -> bool {
        self.decode == TransferFunction::GAMMA22
            && (self.scale - 1.0).abs() <= IDENTITY_TOLERANCE
            && self.ootf.is_none()
            && self.tone.is_none()
            && self.encoding == Encoding::Gamma22
            && self.matrix.is_identity(IDENTITY_TOLERANCE)
    }
}
