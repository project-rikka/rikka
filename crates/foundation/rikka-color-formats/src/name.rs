use rikka_i18n::{Label, tr};

use crate::rgb::Rgb;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColorName {
    White,
    Black,
    LightGray,
    Gray,
    DarkGray,
    Coral,
    Rose,
    LightOrange,
    Tan,
    LightYellow,
    LightGreen,
    Lime,
    Aqua,
    SkyBlue,
    LightTurquoise,
    PaleBlue,
    LightBlue,
    IceBlue,
    Periwinkle,
    Lavender,
    Pink,
    Red,
    Orange,
    Brown,
    Gold,
    Yellow,
    OliveGreen,
    Green,
    BrightGreen,
    Teal,
    Turquoise,
    Blue,
    BlueGray,
    Indigo,
    Purple,
    DarkRed,
    DarkYellow,
    DarkGreen,
    DarkTeal,
    DarkBlue,
    DarkPurple,
    Plum,
}

const HUE_LIMITS: [[u16; 23]; 5] = [
    [
        8, 0, 0, 44, 0, 0, 0, 63, 0, 0, 122, 0, 134, 0, 0, 0, 0, 166, 176, 241, 0, 256, 0,
    ],
    [
        0, 10, 0, 32, 46, 0, 0, 0, 61, 0, 106, 0, 136, 144, 0, 0, 0, 158, 166, 241, 0, 0, 256,
    ],
    [
        0, 8, 0, 0, 39, 46, 0, 0, 0, 71, 120, 0, 131, 144, 0, 0, 163, 0, 177, 211, 249, 0, 256,
    ],
    [
        0, 11, 26, 0, 0, 38, 45, 0, 0, 56, 100, 121, 129, 0, 140, 0, 180, 0, 0, 224, 241, 0, 256,
    ],
    [
        0, 13, 27, 0, 0, 36, 45, 0, 0, 59, 118, 0, 127, 136, 142, 0, 185, 0, 0, 216, 239, 0, 256,
    ],
];

const DARK_BELOW: [u8; 23] = [
    130, 100, 115, 100, 100, 100, 110, 75, 100, 90, 100, 100, 100, 100, 80, 100, 100, 100, 100,
    100, 100, 100, 100,
];

const LIGHT_ABOVE: [u8; 23] = [
    170, 170, 170, 155, 170, 170, 170, 170, 170, 115, 170, 170, 170, 170, 170, 170, 170, 170, 150,
    150, 170, 140, 165,
];

const LIGHT: [ColorName; 23] = [
    ColorName::Coral,
    ColorName::Rose,
    ColorName::LightOrange,
    ColorName::Tan,
    ColorName::Tan,
    ColorName::LightYellow,
    ColorName::LightYellow,
    ColorName::Tan,
    ColorName::LightGreen,
    ColorName::Lime,
    ColorName::LightGreen,
    ColorName::LightGreen,
    ColorName::Aqua,
    ColorName::SkyBlue,
    ColorName::LightTurquoise,
    ColorName::PaleBlue,
    ColorName::LightBlue,
    ColorName::IceBlue,
    ColorName::Periwinkle,
    ColorName::Lavender,
    ColorName::Pink,
    ColorName::Tan,
    ColorName::Rose,
];

const MID: [ColorName; 23] = [
    ColorName::Coral,
    ColorName::Red,
    ColorName::Orange,
    ColorName::Brown,
    ColorName::Tan,
    ColorName::Gold,
    ColorName::Yellow,
    ColorName::OliveGreen,
    ColorName::OliveGreen,
    ColorName::Green,
    ColorName::Green,
    ColorName::BrightGreen,
    ColorName::Teal,
    ColorName::Aqua,
    ColorName::Turquoise,
    ColorName::PaleBlue,
    ColorName::Blue,
    ColorName::BlueGray,
    ColorName::Indigo,
    ColorName::Purple,
    ColorName::Pink,
    ColorName::Brown,
    ColorName::Red,
];

const DARK: [ColorName; 23] = [
    ColorName::Brown,
    ColorName::DarkRed,
    ColorName::Brown,
    ColorName::Brown,
    ColorName::Brown,
    ColorName::DarkYellow,
    ColorName::DarkYellow,
    ColorName::Brown,
    ColorName::DarkGreen,
    ColorName::DarkGreen,
    ColorName::DarkGreen,
    ColorName::DarkGreen,
    ColorName::DarkTeal,
    ColorName::DarkTeal,
    ColorName::DarkTeal,
    ColorName::DarkBlue,
    ColorName::DarkBlue,
    ColorName::BlueGray,
    ColorName::Indigo,
    ColorName::DarkPurple,
    ColorName::Plum,
    ColorName::Brown,
    ColorName::DarkRed,
];

impl ColorName {
    #[must_use]
    pub fn from_rgb(rgb: Rgb) -> ColorName {
        let hsl = rgb.hsl();
        let hue = match hsl.hue == 0.0 {
            true => 0.0,
            false => hsl.hue / 360.0 * 255.0,
        };
        let saturation = hsl.saturation * 255.0;
        let lightness = hsl.lightness * 255.0;

        if lightness > 240.0 {
            return ColorName::White;
        }
        if lightness < 20.0 {
            return ColorName::Black;
        }
        if saturation <= 20.0 {
            return match lightness {
                _ if lightness > 170.0 => ColorName::LightGray,
                _ if lightness > 100.0 => ColorName::Gray,
                _ => ColorName::DarkGray,
            };
        }

        let limits = &HUE_LIMITS[match saturation {
            _ if saturation <= 75.0 => 0,
            _ if saturation <= 115.0 => 1,
            _ if saturation <= 150.0 => 2,
            _ if saturation <= 240.0 => 3,
            _ => 4,
        }];
        let bucket = limits
            .iter()
            .position(|limit| hue < f64::from(*limit))
            .unwrap_or(limits.len() - 1);
        match lightness {
            _ if lightness > f64::from(LIGHT_ABOVE[bucket]) => LIGHT[bucket],
            _ if lightness < f64::from(DARK_BELOW[bucket]) => DARK[bucket],
            _ => MID[bucket],
        }
    }

    #[must_use]
    pub fn label(self) -> Label {
        match self {
            ColorName::White => tr!("color.name.white"),
            ColorName::Black => tr!("color.name.black"),
            ColorName::LightGray => tr!("color.name.light-gray"),
            ColorName::Gray => tr!("color.name.gray"),
            ColorName::DarkGray => tr!("color.name.dark-gray"),
            ColorName::Coral => tr!("color.name.coral"),
            ColorName::Rose => tr!("color.name.rose"),
            ColorName::LightOrange => tr!("color.name.light-orange"),
            ColorName::Tan => tr!("color.name.tan"),
            ColorName::LightYellow => tr!("color.name.light-yellow"),
            ColorName::LightGreen => tr!("color.name.light-green"),
            ColorName::Lime => tr!("color.name.lime"),
            ColorName::Aqua => tr!("color.name.aqua"),
            ColorName::SkyBlue => tr!("color.name.sky-blue"),
            ColorName::LightTurquoise => tr!("color.name.light-turquoise"),
            ColorName::PaleBlue => tr!("color.name.pale-blue"),
            ColorName::LightBlue => tr!("color.name.light-blue"),
            ColorName::IceBlue => tr!("color.name.ice-blue"),
            ColorName::Periwinkle => tr!("color.name.periwinkle"),
            ColorName::Lavender => tr!("color.name.lavender"),
            ColorName::Pink => tr!("color.name.pink"),
            ColorName::Red => tr!("color.name.red"),
            ColorName::Orange => tr!("color.name.orange"),
            ColorName::Brown => tr!("color.name.brown"),
            ColorName::Gold => tr!("color.name.gold"),
            ColorName::Yellow => tr!("color.name.yellow"),
            ColorName::OliveGreen => tr!("color.name.olive-green"),
            ColorName::Green => tr!("color.name.green"),
            ColorName::BrightGreen => tr!("color.name.bright-green"),
            ColorName::Teal => tr!("color.name.teal"),
            ColorName::Turquoise => tr!("color.name.turquoise"),
            ColorName::Blue => tr!("color.name.blue"),
            ColorName::BlueGray => tr!("color.name.blue-gray"),
            ColorName::Indigo => tr!("color.name.indigo"),
            ColorName::Purple => tr!("color.name.purple"),
            ColorName::DarkRed => tr!("color.name.dark-red"),
            ColorName::DarkYellow => tr!("color.name.dark-yellow"),
            ColorName::DarkGreen => tr!("color.name.dark-green"),
            ColorName::DarkTeal => tr!("color.name.dark-teal"),
            ColorName::DarkBlue => tr!("color.name.dark-blue"),
            ColorName::DarkPurple => tr!("color.name.dark-purple"),
            ColorName::Plum => tr!("color.name.plum"),
        }
    }
}
