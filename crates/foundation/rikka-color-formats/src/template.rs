use crate::numbers::{decimals, fraction, percent, rounded, written};
use crate::parameter::{Formatter, Parameter};
use crate::rgb::Rgb;

const MARK: char = '%';

#[derive(Clone, Debug, PartialEq, Eq)]
enum Piece {
    Text(String),
    Value(Parameter, Option<Formatter>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Template {
    source: String,
    pieces: Vec<Piece>,
}

impl Template {
    #[must_use]
    pub fn parse(source: &str) -> Template {
        Template {
            source: source.to_owned(),
            pieces: pieces(source),
        }
    }

    #[must_use]
    pub fn render(&self, rgb: Rgb) -> String {
        if self.source.is_empty() {
            return rgb.hex();
        }
        let mut rendered = String::with_capacity(self.source.len() * 2);
        for piece in &self.pieces {
            match piece {
                Piece::Text(text) => rendered.push_str(text),
                Piece::Value(parameter, formatter) => {
                    rendered.push_str(&value(rgb, *parameter, *formatter));
                }
            }
        }
        rendered
    }
}

fn pieces(source: &str) -> Vec<Piece> {
    let characters: Vec<char> = source.chars().collect();
    let mut pieces = Vec::new();
    let mut text = String::new();
    let mut at = 0;
    while at < characters.len() {
        let character = characters[at];
        if character != MARK || at + 2 >= characters.len() {
            text.push(character);
            at += 1;
            continue;
        }
        let code: String = characters[at + 1..at + 3].iter().collect();
        let Some(parameter) = Parameter::from_code(&code) else {
            text.push(character);
            at += 1;
            continue;
        };
        let formatter = match parameter {
            Parameter::Name => None,
            _ => characters
                .get(at + 3)
                .copied()
                .and_then(Formatter::from_letter),
        };
        if !text.is_empty() {
            pieces.push(Piece::Text(std::mem::take(&mut text)));
        }
        pieces.push(Piece::Value(parameter, formatter));
        at += 3 + usize::from(formatter.is_some());
    }
    if !text.is_empty() {
        pieces.push(Piece::Text(text));
    }
    pieces
}

fn value(rgb: Rgb, parameter: Parameter, formatter: Option<Formatter>) -> String {
    match parameter {
        Parameter::Red => byte(rgb.r, formatter),
        Parameter::Green => byte(rgb.g, formatter),
        Parameter::Blue => byte(rgb.b, formatter),
        Parameter::Alpha => byte(u8::MAX, formatter),
        Parameter::Cyan => percent(rgb.cmyk().cyan),
        Parameter::Magenta => percent(rgb.cmyk().magenta),
        Parameter::Yellow => percent(rgb.cmyk().yellow),
        Parameter::BlackKey => percent(rgb.cmyk().black),
        Parameter::Hue => written(rounded(rgb.hsv().hue, 0)),
        Parameter::NaturalHue => rgb.natural().hue,
        Parameter::SaturationHsi => percent(rgb.hsi().saturation),
        Parameter::SaturationHsl => percent(rgb.hsl().saturation),
        Parameter::SaturationHsb => percent(rgb.hsv().saturation),
        Parameter::Brightness | Parameter::Value => percent(rgb.hsv().value),
        Parameter::Intensity => percent(rgb.hsi().intensity),
        Parameter::LightnessHsl => percent(rgb.hsl().lightness),
        Parameter::Whiteness => percent(rgb.hwb().whiteness),
        Parameter::Blackness => percent(rgb.hwb().blackness),
        Parameter::LightnessLab => lab(rgb.lab().lightness, formatter),
        Parameter::ChromaticityA => lab(rgb.lab().a, formatter),
        Parameter::ChromaticityB => lab(rgb.lab().b, formatter),
        Parameter::LightnessOklab => decimals(rgb.oklab().lightness, 2),
        Parameter::OklabA => decimals(rgb.oklab().a, 2),
        Parameter::OklabB => decimals(rgb.oklab().b, 2),
        Parameter::OklchChroma => decimals(rgb.oklch().chroma, 2),
        Parameter::OklchHue => decimals(rgb.oklch().hue, 2),
        Parameter::X => decimals(rgb.xyz().x * 100.0, 4),
        Parameter::Y => decimals(rgb.xyz().y * 100.0, 4),
        Parameter::Z => decimals(rgb.xyz().z * 100.0, 4),
        Parameter::DecimalRgb => {
            (u32::from(rgb.r) << 16 | u32::from(rgb.g) << 8 | u32::from(rgb.b)).to_string()
        }
        Parameter::DecimalBgr => {
            (u32::from(rgb.b) << 16 | u32::from(rgb.g) << 8 | u32::from(rgb.r)).to_string()
        }
        Parameter::Name => rgb.name().label().as_str().to_owned(),
    }
}

fn byte(channel: u8, formatter: Option<Formatter>) -> String {
    let unit = f64::from(channel) / 255.0;
    match formatter {
        Some(Formatter::HexDigit) => format!("{:x}", channel / 16),
        Some(Formatter::HexDigitUpper) => format!("{:X}", channel / 16),
        Some(Formatter::Hex) => format!("{channel:02x}"),
        Some(Formatter::HexUpper) => format!("{channel:02X}"),
        Some(Formatter::Fraction) => fraction(unit, true),
        Some(Formatter::BareFraction) => fraction(unit, false),
        _ => channel.to_string(),
    }
}

fn lab(value: f64, formatter: Option<Formatter>) -> String {
    match formatter {
        Some(Formatter::Integer) => decimals(value, 0),
        _ => decimals(value, 2),
    }
}
