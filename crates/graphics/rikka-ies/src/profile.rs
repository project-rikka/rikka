use crate::error::IesError;
use crate::parse;

/// The LM-63 revision a file declares on its first line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Edition {
    /// No header line: LM-63-1986.
    Lm1986,
    /// `IESNA91`.
    Lm1991,
    /// `IESNA:LM-63-1995`.
    Lm1995,
    /// `IESNA:LM-63-2002`.
    Lm2002,
    /// `IES:LM-63-2019`.
    Lm2019,
}

/// How the horizontal angles of a type C profile cover the full circle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Symmetry {
    /// One horizontal angle: the same in every horizontal direction.
    Rotational,
    /// 0° to 90°: symmetric in each quadrant.
    Quadrant,
    /// 0° to 180°: symmetric about the 0°–180° plane.
    Bilateral,
    /// 90° to 270°: symmetric about the 90°–270° plane.
    Transverse,
    /// 0° to 360°: no symmetry.
    Full,
}

impl Symmetry {
    fn fold(self, horizontal: f32) -> f32 {
        let horizontal = horizontal.rem_euclid(360.0);
        match self {
            Self::Rotational | Self::Full => horizontal,
            Self::Quadrant => {
                let half = match horizontal > 180.0 {
                    true => 360.0 - horizontal,
                    false => horizontal,
                };
                match half > 90.0 {
                    true => 180.0 - half,
                    false => half,
                }
            }
            Self::Bilateral => match horizontal > 180.0 {
                true => 360.0 - horizontal,
                false => horizontal,
            },
            Self::Transverse => match horizontal {
                h if h < 90.0 => 180.0 - h,
                h if h > 270.0 => 540.0 - h,
                h => h,
            },
        }
    }
}

/// A type C luminous intensity distribution read from an LM-63 file.
///
/// Vertical angles run from 0° at the nadir (straight down from the luminaire) to 180° at the
/// zenith; horizontal angles turn counterclockwise seen from above, 0° along the luminaire's
/// length. Intensities are in candela, already scaled by the file's candela multiplier and
/// ballast factor (and, before LM-63-2002, its ballast-lamp photometric factor).
#[derive(Clone, Debug, PartialEq)]
pub struct IesProfile {
    pub(crate) edition: Edition,
    pub(crate) keywords: Vec<(String, String)>,
    pub(crate) lamps: u32,
    pub(crate) lumens_per_lamp: Option<f32>,
    pub(crate) input_watts: f32,
    pub(crate) vertical: Vec<f32>,
    pub(crate) horizontal: Vec<f32>,
    pub(crate) candela: Vec<f32>,
    pub(crate) symmetry: Symmetry,
}

impl IesProfile {
    /// Reads an LM-63 file; text that is not UTF-8 is read as Latin-1.
    pub fn parse(bytes: &[u8]) -> Result<Self, IesError> {
        parse::parse(bytes)
    }

    pub fn edition(&self) -> Edition {
        self.edition
    }

    /// The value of a `[KEYWORD]` line, with `[MORE]` continuations joined by newlines.
    pub fn keyword(&self, name: &str) -> Option<&str> {
        self.keywords
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// Rated lumens of all lamps, or `None` for absolute photometry (LED luminaires).
    pub fn lamp_lumens(&self) -> Option<f32> {
        self.lumens_per_lamp
            .map(|lumens| lumens * self.lamps as f32)
    }

    pub fn input_watts(&self) -> f32 {
        self.input_watts
    }

    pub fn symmetry(&self) -> Symmetry {
        self.symmetry
    }

    /// Vertical angles of the file in degrees, ascending.
    pub fn vertical_angles(&self) -> &[f32] {
        &self.vertical
    }

    /// Horizontal angles of the file in degrees, ascending.
    pub fn horizontal_angles(&self) -> &[f32] {
        &self.horizontal
    }

    /// The intensity in candela at indices into [`Self::horizontal_angles`] and
    /// [`Self::vertical_angles`].
    pub fn tabulated(&self, horizontal: usize, vertical: usize) -> Option<f32> {
        (vertical < self.vertical.len())
            .then(|| {
                self.candela
                    .get(horizontal * self.vertical.len() + vertical)
                    .copied()
            })
            .flatten()
    }

    /// The highest tabulated intensity in candela.
    pub fn peak_candela(&self) -> f32 {
        self.candela.iter().copied().fold(0.0, f32::max)
    }

    /// Candela towards `vertical` and `horizontal` degrees, zero outside the measured vertical
    /// range.
    pub fn candela(&self, vertical: f32, horizontal: f32) -> f32 {
        let Some((low, high, t)) = bracket(&self.vertical, vertical) else {
            return 0.0;
        };
        let at = |row: usize| {
            let row = &self.candela[row * self.vertical.len()..][..self.vertical.len()];
            lerp(row[low], row[high], t)
        };
        let value = match self.symmetry {
            Symmetry::Rotational => at(0),
            symmetry => match bracket(&self.horizontal, symmetry.fold(horizontal)) {
                Some((left, right, s)) => lerp(at(left), at(right), s),
                None => 0.0,
            },
        };
        value.max(0.0)
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn bracket(angles: &[f32], at: f32) -> Option<(usize, usize, f32)> {
    let (&first, &last) = (angles.first()?, angles.last()?);
    if !(first..=last).contains(&at) {
        return None;
    }
    let high = angles.partition_point(|&angle| angle < at).max(1);
    let low = high - 1;
    let span = angles[high] - angles[low];
    let t = match span > 0.0 {
        true => (at - angles[low]) / span,
        false => 0.0,
    };
    Some((low, high, t))
}
