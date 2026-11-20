use rikka_text_unicode::Script;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Direction {
    #[default]
    LeftToRight,
    RightToLeft,
}

impl Direction {
    pub fn from_script(script: Script) -> Option<Self> {
        match script {
            Script::Arabic
            | Script::Hebrew
            | Script::Syriac
            | Script::Thaana
            | Script::Cypriot
            | Script::Kharoshthi
            | Script::Phoenician
            | Script::Nko
            | Script::Lydian
            | Script::Avestan
            | Script::ImperialAramaic
            | Script::InscriptionalPahlavi
            | Script::InscriptionalParthian
            | Script::OldSouthArabian
            | Script::OldTurkic
            | Script::Samaritan
            | Script::Mandaic
            | Script::MeroiticCursive
            | Script::MeroiticHieroglyphs
            | Script::Manichaean
            | Script::MendeKikakui
            | Script::Nabataean
            | Script::OldNorthArabian
            | Script::Palmyrene
            | Script::PsalterPahlavi
            | Script::Hatran
            | Script::Adlam
            | Script::HanifiRohingya
            | Script::OldSogdian
            | Script::Sogdian
            | Script::Elymaic
            | Script::Chorasmian
            | Script::Yezidi
            | Script::OldUyghur
            | Script::Garay
            | Script::Sidetic => Some(Self::RightToLeft),
            Script::OldHungarian | Script::OldItalic | Script::Runic | Script::Tifinagh => None,
            _ => Some(Self::LeftToRight),
        }
    }

    #[inline]
    pub fn is_forward(self) -> bool {
        self == Self::LeftToRight
    }

    #[inline]
    pub fn is_backward(self) -> bool {
        self == Self::RightToLeft
    }

    #[inline]
    pub fn reversed(self) -> Self {
        match self {
            Self::LeftToRight => Self::RightToLeft,
            Self::RightToLeft => Self::LeftToRight,
        }
    }
}
