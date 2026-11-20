use std::f64::consts::PI;

const BICUBIC_SHARPNESS: f64 = -0.5;
const LANCZOS_LOBES: f64 = 3.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Kernel {
    Nearest,
    Bilinear,
    #[default]
    Bicubic,
    Lanczos3,
}

impl Kernel {
    #[must_use]
    pub const fn support(self) -> f64 {
        match self {
            Kernel::Nearest => 0.5,
            Kernel::Bilinear => 1.0,
            Kernel::Bicubic => 2.0,
            Kernel::Lanczos3 => LANCZOS_LOBES,
        }
    }

    #[must_use]
    pub fn weight(self, distance: f64) -> f64 {
        let x = distance.abs();
        match self {
            Kernel::Nearest => f64::from(u8::from(x < 0.5)),
            Kernel::Bilinear => (1.0 - x).max(0.0),
            Kernel::Bicubic => {
                let a = BICUBIC_SHARPNESS;
                if x < 1.0 {
                    ((a + 2.0) * x - (a + 3.0)) * x * x + 1.0
                } else if x < 2.0 {
                    ((a * x - 5.0 * a) * x + 8.0 * a) * x - 4.0 * a
                } else {
                    0.0
                }
            }
            Kernel::Lanczos3 => {
                if x >= LANCZOS_LOBES {
                    0.0
                } else if x < 1e-9 {
                    1.0
                } else {
                    let phase = PI * x;
                    LANCZOS_LOBES * phase.sin() * (phase / LANCZOS_LOBES).sin() / (phase * phase)
                }
            }
        }
    }
}
