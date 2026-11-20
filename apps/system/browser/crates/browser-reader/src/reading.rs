use crate::block::Block;

const WORDS_PER_MINUTE: f64 = 230.0;
const CHARACTERS_PER_MINUTE: f64 = 500.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reading {
    pub words: usize,
    pub characters: usize,
    pub minutes: u32,
}

impl Reading {
    pub(crate) fn measure(blocks: &[Block]) -> Reading {
        let mut counter = Counter::default();
        for block in blocks {
            block.each_text(&mut |text| counter.count(text));
        }
        let minutes = counter.words as f64 / WORDS_PER_MINUTE
            + counter.characters as f64 / CHARACTERS_PER_MINUTE;
        Reading {
            words: counter.words,
            characters: counter.characters,
            minutes: (minutes.ceil() as u32).max(1),
        }
    }
}

#[derive(Default)]
struct Counter {
    words: usize,
    characters: usize,
}

impl Counter {
    fn count(&mut self, text: &str) {
        let mut in_word = false;
        for character in text.chars() {
            if ideographic(character) {
                self.characters += 1;
                self.end_word(&mut in_word);
            } else if character.is_whitespace() {
                self.end_word(&mut in_word);
            } else if character.is_alphanumeric() {
                in_word = true;
            }
        }
        self.end_word(&mut in_word);
    }

    fn end_word(&mut self, in_word: &mut bool) {
        if *in_word {
            self.words += 1;
            *in_word = false;
        }
    }
}

fn ideographic(character: char) -> bool {
    matches!(
        character as u32,
        0x3040..=0x30FF
            | 0x31F0..=0x31FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0xFF66..=0xFF9D
            | 0x20000..=0x3134F
    )
}
