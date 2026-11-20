use crate::properties::{BracketType, properties};
use crate::tables::bidi::{BRACKET_SOURCES, BRACKET_TARGETS, MIRROR_SOURCES, MIRROR_TARGETS};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PairedBracket {
    pub pair: char,
    pub kind: BracketType,
}

#[inline]
pub fn is_bidi_mirrored(character: char) -> bool {
    properties(character).is_bidi_mirrored()
}

pub fn mirroring_glyph(character: char) -> Option<char> {
    if !is_bidi_mirrored(character) {
        return None;
    }
    pair_in(&MIRROR_SOURCES, &MIRROR_TARGETS, character)
}

pub fn paired_bracket(character: char) -> Option<PairedBracket> {
    let kind = properties(character).bracket_type()?;
    let pair = pair_in(&BRACKET_SOURCES, &BRACKET_TARGETS, character)?;
    Some(PairedBracket { pair, kind })
}

fn pair_in(sources: &[u32], targets: &[u32], character: char) -> Option<char> {
    let index = sources.binary_search(&(character as u32)).ok()?;
    char::from_u32(targets[index])
}
