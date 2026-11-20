use crate::tables::enums::Script;

const THAI_PAIYANNOI: char = '\u{0E2F}';
const THAI_MAIYAMOK: char = '\u{0E46}';

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Language {
    Thai,
    Lao,
    Khmer,
    Burmese,
}

impl Language {
    #[inline]
    pub(crate) fn from_script(script: Script) -> Option<Self> {
        match script {
            Script::Thai => Some(Self::Thai),
            Script::Lao => Some(Self::Lao),
            Script::Khmer => Some(Self::Khmer),
            Script::Myanmar => Some(Self::Burmese),
            _ => None,
        }
    }

    pub(crate) fn shortest_divisible_span(self) -> usize {
        match self {
            Self::Thai => 5,
            Self::Lao | Self::Khmer | Self::Burmese => 4,
        }
    }

    pub(crate) fn can_end_word(self, character: char) -> bool {
        match self {
            Self::Thai => !matches!(character, '\u{0E31}' | '\u{0E40}'..='\u{0E44}'),
            Self::Lao => !matches!(character, '\u{0EC0}'..='\u{0EC4}'),
            Self::Khmer => character != '\u{17D2}',
            Self::Burmese => true,
        }
    }

    pub(crate) fn can_begin_word(self, character: char) -> bool {
        match self {
            Self::Thai => matches!(character, '\u{0E01}'..='\u{0E2E}' | '\u{0E40}'..='\u{0E44}'),
            Self::Lao => matches!(
                character,
                '\u{0E81}'..='\u{0EAE}' | '\u{0EDC}'..='\u{0EDD}' | '\u{0EC0}'..='\u{0EC4}'
            ),
            Self::Khmer => matches!(character, '\u{1780}'..='\u{17B3}'),
            Self::Burmese => matches!(character, '\u{1000}'..='\u{102A}'),
        }
    }

    pub(crate) fn has_suffixes(self) -> bool {
        self == Self::Thai
    }
}

pub(crate) fn is_suffix(character: char) -> bool {
    matches!(character, THAI_PAIYANNOI | THAI_MAIYAMOK)
}

pub(crate) fn is_elision(character: char) -> bool {
    character == THAI_PAIYANNOI
}

pub(crate) fn is_repetition(character: char) -> bool {
    character == THAI_MAIYAMOK
}
