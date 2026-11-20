use crate::key::Key;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BlendMode {
    PassThrough,
    #[default]
    Normal,
    Dissolve,
    Darken,
    Multiply,
    ColorBurn,
    LinearBurn,
    DarkerColor,
    Lighten,
    Screen,
    ColorDodge,
    LinearDodge,
    LighterColor,
    Overlay,
    SoftLight,
    HardLight,
    VividLight,
    LinearLight,
    PinLight,
    HardMix,
    Difference,
    Exclusion,
    Subtract,
    Divide,
    Hue,
    Saturation,
    Color,
    Luminosity,
    Unknown(Key),
}

const TABLE: [(BlendMode, &[u8; 4], &str, &str); 28] = [
    (
        BlendMode::PassThrough,
        b"pass",
        "passThrough",
        "passThrough",
    ),
    (BlendMode::Normal, b"norm", "Nrml", "normal"),
    (BlendMode::Dissolve, b"diss", "Dslv", "dissolve"),
    (BlendMode::Darken, b"dark", "Drkn", "darken"),
    (BlendMode::Multiply, b"mul ", "Mltp", "multiply"),
    (BlendMode::ColorBurn, b"idiv", "CBrn", "colorBurn"),
    (BlendMode::LinearBurn, b"lbrn", "linearBurn", "linearBurn"),
    (
        BlendMode::DarkerColor,
        b"dkCl",
        "darkerColor",
        "darkerColor",
    ),
    (BlendMode::Lighten, b"lite", "Lghn", "lighten"),
    (BlendMode::Screen, b"scrn", "Scrn", "screen"),
    (BlendMode::ColorDodge, b"div ", "CDdg", "colorDodge"),
    (
        BlendMode::LinearDodge,
        b"lddg",
        "linearDodge",
        "linearDodge",
    ),
    (
        BlendMode::LighterColor,
        b"lgCl",
        "lighterColor",
        "lighterColor",
    ),
    (BlendMode::Overlay, b"over", "Ovrl", "overlay"),
    (BlendMode::SoftLight, b"sLit", "SftL", "softLight"),
    (BlendMode::HardLight, b"hLit", "HrdL", "hardLight"),
    (BlendMode::VividLight, b"vLit", "vividLight", "vividLight"),
    (
        BlendMode::LinearLight,
        b"lLit",
        "linearLight",
        "linearLight",
    ),
    (BlendMode::PinLight, b"pLit", "pinLight", "pinLight"),
    (BlendMode::HardMix, b"hMix", "hardMix", "hardMix"),
    (BlendMode::Difference, b"diff", "Dfrn", "difference"),
    (BlendMode::Exclusion, b"smud", "Xclu", "exclusion"),
    (BlendMode::Subtract, b"fsub", "blendSubtraction", "subtract"),
    (BlendMode::Divide, b"fdiv", "blendDivide", "divide"),
    (BlendMode::Hue, b"hue ", "H   ", "hue"),
    (BlendMode::Saturation, b"sat ", "Strt", "saturation"),
    (BlendMode::Color, b"colr", "Clr ", "color"),
    (BlendMode::Luminosity, b"lum ", "Lmns", "luminosity"),
];

impl BlendMode {
    pub const ALL: [BlendMode; 28] = {
        let mut all = [BlendMode::Normal; 28];
        let mut at = 0;
        while at < all.len() {
            all[at] = TABLE[at].0;
            at += 1;
        }
        all
    };

    pub fn from_key(key: Key) -> Self {
        TABLE
            .iter()
            .find(|(_, code, _, _)| **code == key.0)
            .map_or(BlendMode::Unknown(key), |entry| entry.0)
    }

    pub fn key(self) -> Key {
        match self {
            BlendMode::Unknown(key) => key,
            mode => TABLE
                .iter()
                .find(|entry| entry.0 == mode)
                .map_or(Key(*b"norm"), |entry| Key(*entry.1)),
        }
    }

    pub fn from_descriptor_value(value: &str) -> Self {
        if value == "Sbtr" {
            return BlendMode::Subtract;
        }
        TABLE
            .iter()
            .find(|entry| entry.2 == value || entry.3 == value)
            .map_or(BlendMode::Normal, |entry| entry.0)
    }

    pub fn descriptor_value(self) -> &'static str {
        TABLE
            .iter()
            .find(|entry| entry.0 == self)
            .map_or("Nrml", |entry| entry.2)
    }

    pub fn is_separable(self) -> bool {
        !matches!(
            self,
            BlendMode::Hue
                | BlendMode::Saturation
                | BlendMode::Color
                | BlendMode::Luminosity
                | BlendMode::DarkerColor
                | BlendMode::LighterColor
        )
    }
}
