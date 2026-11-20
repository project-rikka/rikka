use rikka_image::ColorScheme;

use crate::target::Target;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Quality {
    #[default]
    Standard,
    High,
    Best,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Scale {
    #[default]
    Original,
    Half,
    LongestSide(u32),
    ShortSide(u32),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Tuning {
    pub quality: Quality,
    pub scale: Scale,
    pub scheme: ColorScheme,
}

impl Tuning {
    #[must_use]
    pub fn suited(target: Target, scheme: ColorScheme) -> Tuning {
        Tuning {
            quality: Quality::Standard,
            scale: target.sizes().first().copied().unwrap_or_default(),
            scheme,
        }
    }
}

impl Quality {
    pub(crate) const fn pick<T: Copy>(self, standard: T, high: T, best: T) -> T {
        match self {
            Quality::Standard => standard,
            Quality::High => high,
            Quality::Best => best,
        }
    }
}

impl Scale {
    #[must_use]
    pub fn fit(self, width: u32, height: u32) -> (u32, u32) {
        let factor = match self {
            Scale::Original => return (width, height),
            Scale::Half => 0.5,
            Scale::LongestSide(most) => ratio(most, width.max(height)),
            Scale::ShortSide(most) => ratio(most, width.min(height)),
        };
        if factor >= 1.0 {
            return (width, height);
        }
        let side = |length: u32| ((f64::from(length) * factor).round() as u32).max(1);
        (side(width), side(height))
    }
}

fn ratio(most: u32, side: u32) -> f64 {
    f64::from(most) / f64::from(side.max(1))
}
