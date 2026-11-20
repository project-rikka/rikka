use super::language::Language;
use crate::tables::dictionaries::{
    DICTIONARY_ACCEPTING, DICTIONARY_LABEL_MASK, DICTIONARY_ROOTS, DICTIONARY_SLOTS,
    DICTIONARY_SYMBOLS, DICTIONARY_TARGET_SHIFT, DICTIONARY_WINDOW, DICTIONARY_WINDOW_STARTS,
};

pub(crate) const MAX_CANDIDATES: usize = 20;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Lexicon {
    window_start: u32,
    root: u32,
    symbols: &'static [u8],
    slots: &'static [u8],
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Matches {
    pub(crate) count: usize,
    pub(crate) prefix: usize,
    pub(crate) lengths: [u16; MAX_CANDIDATES],
    pub(crate) characters: [u8; MAX_CANDIDATES],
}

impl Lexicon {
    pub(crate) fn for_language(language: Language) -> Self {
        let index = language as usize;
        let window = DICTIONARY_WINDOW as usize;
        Self {
            window_start: DICTIONARY_WINDOW_STARTS[index],
            root: DICTIONARY_ROOTS[index],
            symbols: &DICTIONARY_SYMBOLS[index * window..(index + 1) * window],
            slots: DICTIONARY_SLOTS[index],
        }
    }

    #[inline]
    fn symbol(&self, character: char) -> u32 {
        let offset = (character as u32).wrapping_sub(self.window_start);
        self.symbols
            .get(offset as usize)
            .map_or(0, |&symbol| u32::from(symbol))
    }

    #[inline]
    fn edge(&self, base: u32, character: char) -> Option<u32> {
        let symbol = self.symbol(character);
        let index = (base + symbol) as usize * 4;
        let bytes = self.slots.get(index..index + 4)?;
        let slot = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        (symbol != 0 && slot & DICTIONARY_LABEL_MASK == symbol).then_some(slot)
    }

    pub(crate) fn matches(&self, text: &str, start: usize, end: usize, found: &mut Matches) {
        let mut base = self.root;
        let mut consumed = 0;
        found.count = 0;
        for (offset, character) in text[start..end].char_indices() {
            consumed += 1;
            let Some(slot) = self.edge(base, character) else {
                break;
            };
            if slot & DICTIONARY_ACCEPTING != 0 && found.count < MAX_CANDIDATES {
                found.lengths[found.count] = (offset + character.len_utf8()) as u16;
                found.characters[found.count] = consumed as u8;
                found.count += 1;
            }
            base = slot >> DICTIONARY_TARGET_SHIFT;
            if base == 0 {
                break;
            }
        }
        found.prefix = consumed;
    }
}
