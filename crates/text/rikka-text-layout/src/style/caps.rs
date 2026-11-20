use rikka_text_font::Tag;
use rikka_text_shape::Feature;

const SMALL: Feature = enabled(b"smcp");
const SMALL_FROM_CAPITALS: Feature = enabled(b"c2sc");
const PETITE: Feature = enabled(b"pcap");
const PETITE_FROM_CAPITALS: Feature = enabled(b"c2pc");
const UNICASE: Feature = enabled(b"unic");
const TITLING: Feature = enabled(b"titl");

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Caps {
    #[default]
    Normal,
    Small,
    AllSmall,
    Petite,
    AllPetite,
    Unicase,
    Titling,
}

impl Caps {
    pub(crate) fn features(self) -> &'static [Feature] {
        match self {
            Caps::Normal => &[],
            Caps::Small => &[SMALL],
            Caps::AllSmall => &[SMALL, SMALL_FROM_CAPITALS],
            Caps::Petite => &[PETITE],
            Caps::AllPetite => &[PETITE, PETITE_FROM_CAPITALS],
            Caps::Unicase => &[UNICASE],
            Caps::Titling => &[TITLING],
        }
    }
}

const fn enabled(tag: &[u8; 4]) -> Feature {
    Feature {
        tag: Tag::new(tag),
        value: 1,
        start: 0,
        end: u32::MAX,
    }
}
