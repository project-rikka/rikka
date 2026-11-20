use super::hangul;
use crate::tables::normalization::{
    COMPATIBILITY, COMPATIBILITY_PARTS, COMPATIBILITY_STARTS, COMPOSITION_STARTS, COMPOSITIONS,
    DECOMPOSITIONS, NORMALIZATION,
};

const MAX_DECOMPOSITION: usize = 4;

pub fn decompose(character: char) -> Option<(char, Option<char>)> {
    let codepoint = character as u32;
    if let Some((first, second)) = hangul::decompose(codepoint) {
        return Some((char::from_u32(first)?, char::from_u32(second)));
    }
    let index = (NORMALIZATION.get(codepoint) & 0xFFFF) as usize;
    if index == 0 {
        return None;
    }
    let first = char::from_u32(DECOMPOSITIONS[2 * index - 2])?;
    let second = DECOMPOSITIONS[2 * index - 1];
    Some((
        first,
        (second != 0).then(|| char::from_u32(second)).flatten(),
    ))
}

pub fn compose(first: char, second: char) -> Option<char> {
    let (first, second) = (first as u32, second as u32);
    if let Some(syllable) = hangul::compose(first, second) {
        return syllable.and_then(char::from_u32);
    }
    let group = (NORMALIZATION.get(first) >> 16) as usize;
    if group == 0 {
        return None;
    }
    let start = usize::from(COMPOSITION_STARTS[group - 1]);
    let end = usize::from(COMPOSITION_STARTS[group]);
    COMPOSITIONS[2 * start..2 * end]
        .as_chunks::<2>()
        .0
        .iter()
        .find(|[candidate, _]| *candidate == second)
        .and_then(|&[_, composite]| char::from_u32(composite))
}

#[derive(Clone, Debug)]
pub struct Decomposition {
    characters: [char; MAX_DECOMPOSITION],
    position: u8,
    len: u8,
}

impl Iterator for Decomposition {
    type Item = char;

    #[inline]
    fn next(&mut self) -> Option<char> {
        let character =
            *self.characters[..usize::from(self.len)].get(usize::from(self.position))?;
        self.position += 1;
        Some(character)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = usize::from(self.len - self.position);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for Decomposition {}

pub fn canonical_decomposition(character: char) -> Decomposition {
    let mut decomposition = Decomposition {
        characters: ['\0'; MAX_DECOMPOSITION],
        position: 0,
        len: 0,
    };
    push_recursively(&mut decomposition, character);
    decomposition
}

fn push_recursively(decomposition: &mut Decomposition, character: char) {
    match decompose(character) {
        Some((first, second)) => {
            push_recursively(decomposition, first);
            if let Some(second) = second {
                push_recursively(decomposition, second);
            }
        }
        None => {
            decomposition.characters[usize::from(decomposition.len)] = character;
            decomposition.len += 1;
        }
    }
}

pub(crate) fn compatibility_decomposition(character: char) -> Option<&'static [u32]> {
    let index = usize::from(COMPATIBILITY.get(character as u32));
    if index == 0 {
        return None;
    }
    let start = usize::from(COMPATIBILITY_STARTS[index - 1]);
    let end = usize::from(COMPATIBILITY_STARTS[index]);
    Some(&COMPATIBILITY_PARTS[start..end])
}
