use std::fmt;
use std::str::FromStr;

use rikka_types::Color;

use crate::error::{Error, Result};
use crate::name::ColorName;
use crate::shades::Shades;
use crate::spaces::{Cmyk, Hsi, Hsl, Hsv, Hwb, Lab, Natural, Oklab, Oklch, Xyz};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const MOCCASIN: Rgb = Rgb::new(255, 228, 181);

    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Rgb {
        Rgb { r, g, b }
    }

    #[must_use]
    pub fn parse_hex(typed: &str) -> Option<Rgb> {
        let typed = typed.trim();
        let digits = typed.strip_prefix('#').unwrap_or(typed);
        if !matches!(digits.len(), 3 | 6) {
            return None;
        }
        Color::parse_hex(&format!("#{digits}")).map(Rgb::from)
    }

    #[must_use]
    pub fn hex(self) -> String {
        format!("{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    #[must_use]
    pub fn color(self) -> Color {
        Color::rgb8(self.r, self.g, self.b)
    }

    #[must_use]
    pub fn hsv(self) -> Hsv {
        Hsv::from_rgb(self)
    }

    #[must_use]
    pub fn hsl(self) -> Hsl {
        Hsl::from_rgb(self)
    }

    #[must_use]
    pub fn hsi(self) -> Hsi {
        Hsi::from_rgb(self)
    }

    #[must_use]
    pub fn hwb(self) -> Hwb {
        Hwb::from_rgb(self)
    }

    #[must_use]
    pub fn cmyk(self) -> Cmyk {
        Cmyk::from_rgb(self)
    }

    #[must_use]
    pub fn xyz(self) -> Xyz {
        Xyz::from_rgb(self)
    }

    #[must_use]
    pub fn lab(self) -> Lab {
        Lab::from_rgb(self)
    }

    #[must_use]
    pub fn oklab(self) -> Oklab {
        Oklab::from_rgb(self)
    }

    #[must_use]
    pub fn oklch(self) -> Oklch {
        Oklch::from_rgb(self)
    }

    #[must_use]
    pub fn natural(self) -> Natural {
        Natural::from_rgb(self)
    }

    #[must_use]
    pub fn name(self) -> ColorName {
        ColorName::from_rgb(self)
    }

    #[must_use]
    pub fn shades(self) -> Shades {
        Shades::new(self)
    }

    pub(crate) fn unit(self) -> [f64; 3] {
        [self.r, self.g, self.b].map(|channel| f64::from(channel) / 255.0)
    }

    pub(crate) fn lowest(self) -> u8 {
        self.r.min(self.g).min(self.b)
    }

    pub(crate) fn highest(self) -> u8 {
        self.r.max(self.g).max(self.b)
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.hex())
    }
}

impl FromStr for Rgb {
    type Err = Error;

    fn from_str(typed: &str) -> Result<Rgb> {
        Rgb::parse_hex(typed).ok_or_else(|| Error::NotHex(typed.to_owned()))
    }
}

impl serde::Serialize for Rgb {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.hex())
    }
}

impl<'de> serde::Deserialize<'de> for Rgb {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Rgb, D::Error> {
        let typed = String::deserialize(deserializer)?;
        typed.parse().map_err(serde::de::Error::custom)
    }
}

impl From<Color> for Rgb {
    fn from(color: Color) -> Rgb {
        let [r, g, b, _] = color.to_rgba8();
        Rgb::new(r, g, b)
    }
}
