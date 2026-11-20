use rikka_text_unicode::Script;

const DISPLAYED: [Script; 30] = [
    Script::Latin,
    Script::Greek,
    Script::Cyrillic,
    Script::Armenian,
    Script::Hebrew,
    Script::Arabic,
    Script::Thaana,
    Script::Devanagari,
    Script::Bengali,
    Script::Gurmukhi,
    Script::Gujarati,
    Script::Oriya,
    Script::Tamil,
    Script::Telugu,
    Script::Kannada,
    Script::Malayalam,
    Script::Sinhala,
    Script::Thai,
    Script::Lao,
    Script::Tibetan,
    Script::Myanmar,
    Script::Georgian,
    Script::Hangul,
    Script::Ethiopic,
    Script::Khmer,
    Script::Mongolian,
    Script::Hiragana,
    Script::Katakana,
    Script::Bopomofo,
    Script::Han,
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Scripts(u32);

impl Scripts {
    pub(crate) const fn of(scripts: &[Script]) -> Scripts {
        let mut mask = 0;
        let mut index = 0;
        while index < scripts.len() {
            let mut position = 0;
            while position < DISPLAYED.len() {
                if DISPLAYED[position] as u8 == scripts[index] as u8 {
                    mask |= 1 << position;
                }
                position += 1;
            }
            index += 1;
        }
        Scripts(mask)
    }

    pub(crate) fn add(&mut self, script: Script) -> bool {
        match DISPLAYED.iter().position(|shown| *shown == script) {
            Some(position) => {
                self.0 |= 1 << position;
                true
            }
            None => false,
        }
    }

    pub(crate) fn count(self) -> u32 {
        self.0.count_ones()
    }

    pub(crate) fn within(self, allowed: Scripts) -> bool {
        self.0 & !allowed.0 == 0
    }
}

pub(crate) fn script(character: char) -> Script {
    rikka_text_unicode::script(character)
}

pub(crate) fn is_kana(character: char) -> bool {
    matches!(script(character), Script::Hiragana | Script::Katakana)
}

pub(crate) fn is_cjk(character: char) -> bool {
    matches!(
        script(character),
        Script::Han | Script::Hiragana | Script::Katakana | Script::Hangul | Script::Bopomofo
    ) || matches!(character, '\u{3005}' | '\u{30FC}' | '\u{FF70}')
}
