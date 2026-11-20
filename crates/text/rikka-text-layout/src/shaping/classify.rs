use rikka_text_unicode::{BidiClass, GeneralCategory, LineBreakClass, properties};

use crate::shaping::columns::flag::{LINE_BREAK, SEPARATOR, TAB, WHITESPACE, WORD_SEPARATOR};

pub(crate) const INVISIBLE: u8 = 0x80;

const ASCII: [u8; 128] = ascii_table();

const fn ascii_table() -> [u8; 128] {
    let mut table = [0u8; 128];
    let mut byte = 0;
    while byte < 128 {
        table[byte] = match byte {
            0x09 => TAB | SEPARATOR | WHITESPACE | INVISIBLE,
            0x0A | 0x0D => SEPARATOR | WHITESPACE | LINE_BREAK | INVISIBLE,
            0x0B => SEPARATOR | WHITESPACE | LINE_BREAK | INVISIBLE,
            0x0C => WHITESPACE | LINE_BREAK | INVISIBLE,
            0x1C..=0x1E => SEPARATOR | WHITESPACE | INVISIBLE,
            0x1F => SEPARATOR | WHITESPACE | INVISIBLE,
            0x00..=0x1B | 0x7F => WHITESPACE | INVISIBLE,
            0x20 => WHITESPACE | WORD_SEPARATOR,
            _ => 0,
        };
        byte += 1;
    }
    table
}

#[inline]
pub(crate) fn classify(text: &str, offset: usize) -> u8 {
    let bytes = text.as_bytes();
    match bytes.get(offset) {
        Some(&byte) if byte < 0x80 => ASCII[usize::from(byte)],
        Some(_) => text[offset..].chars().next().map_or(0, classify_char),
        None => 0,
    }
}

fn classify_char(character: char) -> u8 {
    let properties = properties(character);
    let mut flags = match properties.bidi_class() {
        BidiClass::WhiteSpace
        | BidiClass::BoundaryNeutral
        | BidiClass::FirstStrongIsolate
        | BidiClass::LeftToRightIsolate
        | BidiClass::RightToLeftIsolate
        | BidiClass::PopDirectionalIsolate
        | BidiClass::LeftToRightEmbedding
        | BidiClass::RightToLeftEmbedding
        | BidiClass::LeftToRightOverride
        | BidiClass::RightToLeftOverride
        | BidiClass::PopDirectionalFormat => WHITESPACE,
        BidiClass::SegmentSeparator | BidiClass::ParagraphSeparator => SEPARATOR | WHITESPACE,
        _ => 0,
    };
    if matches!(
        character,
        '\u{A0}' | '\u{1361}' | '\u{10100}' | '\u{10101}' | '\u{1039F}' | '\u{1091F}'
    ) {
        flags |= WORD_SEPARATOR;
    }
    if matches!(
        properties.line_break_class(),
        LineBreakClass::MandatoryBreak
            | LineBreakClass::CarriageReturn
            | LineBreakClass::LineFeed
            | LineBreakClass::NextLine
    ) {
        flags |= LINE_BREAK;
    }
    if matches!(
        properties.general_category(),
        GeneralCategory::Control
            | GeneralCategory::LineSeparator
            | GeneralCategory::ParagraphSeparator
    ) {
        flags |= INVISIBLE;
    }
    flags
}
