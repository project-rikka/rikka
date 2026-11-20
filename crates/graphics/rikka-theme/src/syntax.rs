use rikka_types::Color;

use crate::ansi::{Hues, Ink};
use crate::tone;
use crate::variant::Variant;

const INK_FLOOR: f32 = 56.0;
const INK_CEILING: f32 = 48.0;

const DARK_COMMENT: std::ops::RangeInclusive<f32> = 36.0..=52.0;
const LIGHT_COMMENT: std::ops::RangeInclusive<f32> = 52.0..=66.0;
const ERROR_CHROMA: f64 = 1.35;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Token {
    Comment,
    Keyword,
    Operator,
    Punctuation,
    String,
    Escape,
    Number,
    Constant,
    Function,
    Type,
    Variable,
    Property,
    Parameter,
    Attribute,
    Tag,
    Namespace,
    Error,
}

impl Token {
    pub const ALL: [Token; 17] = [
        Token::Comment,
        Token::Keyword,
        Token::Operator,
        Token::Punctuation,
        Token::String,
        Token::Escape,
        Token::Number,
        Token::Constant,
        Token::Function,
        Token::Type,
        Token::Variable,
        Token::Property,
        Token::Parameter,
        Token::Attribute,
        Token::Tag,
        Token::Namespace,
        Token::Error,
    ];

    fn slot(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Syntax {
    colors: [Color; Token::ALL.len()],
}

impl Syntax {
    #[must_use]
    pub fn color(&self, token: Token) -> Color {
        self.colors[token.slot()]
    }

    pub(crate) fn derive(hues: &Hues, ink: &Ink, variant: Variant) -> Syntax {
        let read = |color: Color| match variant.is_dark() {
            true => tone::with_tone(color, tone::measure(color).max(INK_FLOOR)),
            false => tone::with_tone(color, tone::measure(color).min(INK_CEILING)),
        };
        let comment = |color: Color| {
            let band = match variant.is_dark() {
                true => DARK_COMMENT,
                false => LIGHT_COMMENT,
            };
            tone::with_tone(
                color,
                tone::measure(color).clamp(*band.start(), *band.end()),
            )
        };
        let mut colors = [ink.text; Token::ALL.len()];
        let mut paint = |token: Token, color: Color| colors[token.slot()] = color;

        paint(Token::Comment, comment(ink.muted));
        paint(Token::Keyword, read(hues.purple));
        paint(Token::Operator, read(hues.cyan));
        paint(Token::Punctuation, read(ink.muted));
        paint(Token::String, read(hues.green));
        paint(Token::Escape, read(hues.magenta));
        paint(Token::Number, read(hues.orange));
        paint(Token::Constant, read(hues.orange));
        paint(Token::Function, read(hues.blue));
        paint(Token::Type, read(hues.yellow));
        paint(Token::Variable, ink.text);
        paint(Token::Property, read(hues.red));
        paint(Token::Parameter, ink.subtext);
        paint(Token::Attribute, read(hues.yellow));
        paint(Token::Tag, read(hues.red));
        paint(Token::Namespace, read(hues.cyan));
        paint(
            Token::Error,
            tone::shifted(read(hues.red), tone::measure(read(hues.red)), |chroma| {
                chroma * ERROR_CHROMA
            }),
        );

        Syntax { colors }
    }
}
