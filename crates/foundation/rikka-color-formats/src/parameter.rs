use rikka_i18n::{Label, tr};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Parameter {
    Red,
    Green,
    Blue,
    Alpha,
    Cyan,
    Magenta,
    Yellow,
    BlackKey,
    Hue,
    NaturalHue,
    SaturationHsi,
    SaturationHsl,
    SaturationHsb,
    Brightness,
    Intensity,
    LightnessHsl,
    Value,
    Whiteness,
    Blackness,
    LightnessLab,
    ChromaticityA,
    ChromaticityB,
    LightnessOklab,
    OklabA,
    OklabB,
    OklchChroma,
    OklchHue,
    X,
    Y,
    Z,
    DecimalRgb,
    DecimalBgr,
    Name,
}

impl Parameter {
    pub const ALL: [Parameter; 33] = [
        Parameter::Red,
        Parameter::Green,
        Parameter::Blue,
        Parameter::Alpha,
        Parameter::Cyan,
        Parameter::Magenta,
        Parameter::Yellow,
        Parameter::BlackKey,
        Parameter::Hue,
        Parameter::NaturalHue,
        Parameter::SaturationHsi,
        Parameter::SaturationHsl,
        Parameter::SaturationHsb,
        Parameter::Brightness,
        Parameter::Intensity,
        Parameter::LightnessHsl,
        Parameter::Value,
        Parameter::Whiteness,
        Parameter::Blackness,
        Parameter::LightnessLab,
        Parameter::ChromaticityA,
        Parameter::ChromaticityB,
        Parameter::LightnessOklab,
        Parameter::OklabA,
        Parameter::OklabB,
        Parameter::OklchChroma,
        Parameter::OklchHue,
        Parameter::X,
        Parameter::Y,
        Parameter::Z,
        Parameter::DecimalRgb,
        Parameter::DecimalBgr,
        Parameter::Name,
    ];

    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Parameter::Red => "Re",
            Parameter::Green => "Gr",
            Parameter::Blue => "Bl",
            Parameter::Alpha => "Al",
            Parameter::Cyan => "Cy",
            Parameter::Magenta => "Ma",
            Parameter::Yellow => "Ye",
            Parameter::BlackKey => "Bk",
            Parameter::Hue => "Hu",
            Parameter::NaturalHue => "Hn",
            Parameter::SaturationHsi => "Si",
            Parameter::SaturationHsl => "Sl",
            Parameter::SaturationHsb => "Sb",
            Parameter::Brightness => "Br",
            Parameter::Intensity => "In",
            Parameter::LightnessHsl => "Ll",
            Parameter::Value => "Va",
            Parameter::Whiteness => "Wh",
            Parameter::Blackness => "Bn",
            Parameter::LightnessLab => "Lc",
            Parameter::ChromaticityA => "Ca",
            Parameter::ChromaticityB => "Cb",
            Parameter::LightnessOklab => "Lo",
            Parameter::OklabA => "Oa",
            Parameter::OklabB => "Ob",
            Parameter::OklchChroma => "Oc",
            Parameter::OklchHue => "Oh",
            Parameter::X => "Xv",
            Parameter::Y => "Yv",
            Parameter::Z => "Zv",
            Parameter::DecimalRgb => "Dr",
            Parameter::DecimalBgr => "Dv",
            Parameter::Name => "Na",
        }
    }

    #[must_use]
    pub fn from_code(code: &str) -> Option<Parameter> {
        Parameter::ALL
            .into_iter()
            .find(|parameter| parameter.code() == code)
    }

    #[must_use]
    pub fn label(self) -> Label {
        match self {
            Parameter::Red => tr!("color.parameter.red"),
            Parameter::Green => tr!("color.parameter.green"),
            Parameter::Blue => tr!("color.parameter.blue"),
            Parameter::Alpha => tr!("color.parameter.alpha"),
            Parameter::Cyan => tr!("color.parameter.cyan"),
            Parameter::Magenta => tr!("color.parameter.magenta"),
            Parameter::Yellow => tr!("color.parameter.yellow"),
            Parameter::BlackKey => tr!("color.parameter.black-key"),
            Parameter::Hue => tr!("color.parameter.hue"),
            Parameter::NaturalHue => tr!("color.parameter.natural-hue"),
            Parameter::SaturationHsi => tr!("color.parameter.saturation-hsi"),
            Parameter::SaturationHsl => tr!("color.parameter.saturation-hsl"),
            Parameter::SaturationHsb => tr!("color.parameter.saturation-hsb"),
            Parameter::Brightness => tr!("color.parameter.brightness"),
            Parameter::Intensity => tr!("color.parameter.intensity"),
            Parameter::LightnessHsl => tr!("color.parameter.lightness-hsl"),
            Parameter::Value => tr!("color.parameter.value"),
            Parameter::Whiteness => tr!("color.parameter.whiteness"),
            Parameter::Blackness => tr!("color.parameter.blackness"),
            Parameter::LightnessLab => tr!("color.parameter.lightness-lab"),
            Parameter::ChromaticityA => tr!("color.parameter.chromaticity-a"),
            Parameter::ChromaticityB => tr!("color.parameter.chromaticity-b"),
            Parameter::LightnessOklab => tr!("color.parameter.lightness-oklab"),
            Parameter::OklabA => tr!("color.parameter.oklab-a"),
            Parameter::OklabB => tr!("color.parameter.oklab-b"),
            Parameter::OklchChroma => tr!("color.parameter.oklch-chroma"),
            Parameter::OklchHue => tr!("color.parameter.oklch-hue"),
            Parameter::X => tr!("color.parameter.x"),
            Parameter::Y => tr!("color.parameter.y"),
            Parameter::Z => tr!("color.parameter.z"),
            Parameter::DecimalRgb => tr!("color.parameter.decimal-rgb"),
            Parameter::DecimalBgr => tr!("color.parameter.decimal-bgr"),
            Parameter::Name => tr!("color.parameter.name"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Formatter {
    Byte,
    HexDigit,
    HexDigitUpper,
    Hex,
    HexUpper,
    Fraction,
    BareFraction,
    Percent,
    Integer,
    Text,
}

impl Formatter {
    pub const CHANNELS: [Formatter; 7] = [
        Formatter::Byte,
        Formatter::HexDigit,
        Formatter::HexDigitUpper,
        Formatter::Hex,
        Formatter::HexUpper,
        Formatter::Fraction,
        Formatter::BareFraction,
    ];

    pub const LAB: [Formatter; 1] = [Formatter::Integer];

    const ALL: [Formatter; 10] = [
        Formatter::Byte,
        Formatter::HexDigit,
        Formatter::HexDigitUpper,
        Formatter::Hex,
        Formatter::HexUpper,
        Formatter::Fraction,
        Formatter::BareFraction,
        Formatter::Percent,
        Formatter::Integer,
        Formatter::Text,
    ];

    #[must_use]
    pub fn letter(self) -> char {
        match self {
            Formatter::Byte => 'b',
            Formatter::HexDigit => 'h',
            Formatter::HexDigitUpper => 'H',
            Formatter::Hex => 'x',
            Formatter::HexUpper => 'X',
            Formatter::Fraction => 'f',
            Formatter::BareFraction => 'F',
            Formatter::Percent => 'p',
            Formatter::Integer => 'i',
            Formatter::Text => 's',
        }
    }

    pub(crate) fn from_letter(letter: char) -> Option<Formatter> {
        Formatter::ALL
            .into_iter()
            .find(|formatter| formatter.letter() == letter)
    }

    #[must_use]
    pub fn label(self) -> Label {
        match self {
            Formatter::Byte => tr!("color.formatter.byte"),
            Formatter::HexDigit => tr!("color.formatter.hex-digit"),
            Formatter::HexDigitUpper => tr!("color.formatter.hex-digit-upper"),
            Formatter::Hex => tr!("color.formatter.hex"),
            Formatter::HexUpper => tr!("color.formatter.hex-upper"),
            Formatter::Fraction => tr!("color.formatter.fraction"),
            Formatter::BareFraction => tr!("color.formatter.bare-fraction"),
            Formatter::Percent => tr!("color.formatter.percent"),
            Formatter::Integer => tr!("color.formatter.integer"),
            Formatter::Text => tr!("color.formatter.text"),
        }
    }
}
