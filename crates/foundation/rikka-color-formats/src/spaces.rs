use crate::numbers::rounded;
use crate::rgb::Rgb;

const SRGB_TO_XYZ: [[f64; 3]; 3] = [
    [
        0.412_390_799_265_959_5,
        0.357_584_339_383_877_96,
        0.180_480_788_401_834_3,
    ],
    [
        0.212_639_005_871_510_36,
        0.715_168_678_767_755_9,
        0.072_192_315_360_733_72,
    ],
    [
        0.019_330_818_715_591_85,
        0.119_194_779_794_625_99,
        0.950_532_152_249_660_6,
    ],
];

const WHITE: [f64; 3] = [0.950_455_927_051_671_7, 1.0, 1.089_057_750_759_878_4];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsv {
    pub hue: f64,
    pub saturation: f64,
    pub value: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsl {
    pub hue: f64,
    pub saturation: f64,
    pub lightness: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsi {
    pub hue: f64,
    pub saturation: f64,
    pub intensity: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hwb {
    pub hue: f64,
    pub whiteness: f64,
    pub blackness: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cmyk {
    pub cyan: f64,
    pub magenta: f64,
    pub yellow: f64,
    pub black: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xyz {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lab {
    pub lightness: f64,
    pub a: f64,
    pub b: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklab {
    pub lightness: f64,
    pub a: f64,
    pub b: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklch {
    pub lightness: f64,
    pub chroma: f64,
    pub hue: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Natural {
    pub hue: String,
    pub whiteness: f64,
    pub blackness: f64,
}

pub(crate) fn hue(rgb: Rgb) -> f64 {
    let (r, g, b) = (i32::from(rgb.r), i32::from(rgb.g), i32::from(rgb.b));
    if r == g && g == b {
        return 0.0;
    }
    let (lowest, highest) = (i32::from(rgb.lowest()), i32::from(rgb.highest()));
    let delta = (highest - lowest) as f32;
    let sector = if r == highest {
        (g - b) as f32 / delta
    } else if g == highest {
        (b - r) as f32 / delta + 2.0
    } else {
        (r - g) as f32 / delta + 4.0
    };
    let degrees = sector * 60.0;
    f64::from(match degrees < 0.0 {
        true => degrees + 360.0,
        false => degrees,
    })
}

fn extremes(rgb: Rgb) -> (f64, f64) {
    (
        f64::from(rgb.lowest()) / 255.0,
        f64::from(rgb.highest()) / 255.0,
    )
}

impl Hsv {
    pub(crate) fn from_rgb(rgb: Rgb) -> Hsv {
        let (lowest, highest) = extremes(rgb);
        Hsv {
            hue: hue(rgb),
            saturation: match highest == 0.0 {
                true => 0.0,
                false => (highest - lowest) / highest,
            },
            value: highest,
        }
    }

    #[must_use]
    pub fn rgb(self) -> Rgb {
        let Hsv {
            hue,
            saturation,
            value,
        } = self;
        let within = |unit: f64| (0.0..=1.0).contains(&unit);
        if !(0.0..=360.0).contains(&hue) || !within(saturation) || !within(value) {
            return Rgb::new(0, 0, 0);
        }
        let chroma = value * saturation;
        let rising = chroma * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
        let floor = value - chroma;
        let (r, g, b) = match hue {
            _ if hue < 60.0 => (chroma, rising, 0.0),
            _ if hue < 120.0 => (rising, chroma, 0.0),
            _ if hue < 180.0 => (0.0, chroma, rising),
            _ if hue < 240.0 => (0.0, rising, chroma),
            _ if hue < 300.0 => (rising, 0.0, chroma),
            _ => (chroma, 0.0, rising),
        };
        let byte = |unit: f64| ((unit + floor) * 255.0) as u8;
        Rgb::new(byte(r), byte(g), byte(b))
    }
}

impl Hsl {
    pub(crate) fn from_rgb(rgb: Rgb) -> Hsl {
        let (lowest, highest) = extremes(rgb);
        let lightness = (highest + lowest) / 2.0;
        let saturation = if lightness == 0.0 || lowest == highest {
            0.0
        } else if lightness <= 0.5 {
            (highest - lowest) / (highest + lowest)
        } else {
            (highest - lowest) / (2.0 - (highest + lowest))
        };
        Hsl {
            hue: hue(rgb),
            saturation,
            lightness,
        }
    }
}

impl Hsi {
    pub(crate) fn from_rgb(rgb: Rgb) -> Hsi {
        if rgb == Rgb::new(0, 0, 0) {
            return Hsi {
                hue: 0.0,
                saturation: 0.0,
                intensity: 0.0,
            };
        }
        let [r, g, b] = rgb.unit();
        let intensity = (r + g + b) / 3.0;
        let (lowest, _) = extremes(rgb);
        Hsi {
            hue: hue(rgb),
            saturation: 1.0 - lowest / intensity,
            intensity,
        }
    }
}

impl Hwb {
    pub(crate) fn from_rgb(rgb: Rgb) -> Hwb {
        let (lowest, highest) = extremes(rgb);
        Hwb {
            hue: hue(rgb),
            whiteness: lowest,
            blackness: 1.0 - highest,
        }
    }
}

impl Cmyk {
    pub(crate) fn from_rgb(rgb: Rgb) -> Cmyk {
        let black = Cmyk {
            cyan: 0.0,
            magenta: 0.0,
            yellow: 0.0,
            black: 1.0,
        };
        if rgb == Rgb::new(0, 0, 0) {
            return black;
        }
        let [r, g, b] = rgb.unit();
        let key = 1.0 - r.max(g).max(b);
        if 1.0 - key == 0.0 {
            return black;
        }
        let ink = |channel: f64| (1.0 - channel - key) / (1.0 - key);
        Cmyk {
            cyan: ink(r),
            magenta: ink(g),
            yellow: ink(b),
            black: key,
        }
    }
}

fn linear(rgb: Rgb) -> [f64; 3] {
    rgb.unit().map(|channel| match channel > 0.040_45 {
        true => ((channel + 0.055) / 1.055).powf(2.4),
        false => channel / 12.92,
    })
}

fn transform(matrix: &[[f64; 3]; 3], [a, b, c]: [f64; 3]) -> [f64; 3] {
    matrix.map(|[x, y, z]| a * x + b * y + c * z)
}

impl Xyz {
    pub(crate) fn from_rgb(rgb: Rgb) -> Xyz {
        let [x, y, z] = transform(&SRGB_TO_XYZ, linear(rgb));
        Xyz { x, y, z }
    }
}

impl Lab {
    pub(crate) fn from_rgb(rgb: Rgb) -> Lab {
        let Xyz { x, y, z } = Xyz::from_rgb(rgb);
        let delta: f64 = 6.0 / 29.0;
        let slope = (1.0 / 3.0) * delta.powf(-2.0);
        let knee = delta.powf(3.0);
        let curve = |unit: f64| match unit > knee {
            true => unit.powf(1.0 / 3.0),
            false => unit * slope + 16.0 / 116.0,
        };
        let [fx, fy, fz] = [x / WHITE[0], y / WHITE[1], z / WHITE[2]].map(curve);
        Lab {
            lightness: 116.0 * fy - 16.0,
            a: 500.0 * (fx - fy),
            b: 200.0 * (fy - fz),
        }
    }
}

impl Oklab {
    pub(crate) fn from_rgb(rgb: Rgb) -> Oklab {
        let [lightness, a, b] = rikka_types::oklab::from_linear(linear(rgb));
        Oklab { lightness, a, b }
    }
}

impl Oklch {
    pub(crate) fn from_rgb(rgb: Rgb) -> Oklch {
        let Oklab { lightness, a, b } = Oklab::from_rgb(rgb);
        let chroma = (a * a + b * b).sqrt();
        let hue = match rounded(chroma, 3) == 0.0 {
            true => 0.0,
            false => (b.atan2(a) * 180.0 / std::f64::consts::PI + 360.0) % 360.0,
        };
        Oklch {
            lightness,
            chroma,
            hue,
        }
    }
}

impl Natural {
    pub(crate) fn from_rgb(rgb: Rgb) -> Natural {
        let (lowest, highest) = extremes(rgb);
        Natural {
            hue: natural_hue(hue(rgb)),
            whiteness: lowest,
            blackness: 1.0 - highest,
        }
    }
}

fn natural_hue(hue: f64) -> String {
    const LETTERS: [char; 6] = ['R', 'Y', 'G', 'C', 'B', 'M'];
    let sector = ((hue / 60.0) as usize).min(LETTERS.len() - 1);
    let within = hue - sector as f64 * 60.0;
    format!("{}{}", LETTERS[sector], (within / 0.6).round_ties_even())
}
