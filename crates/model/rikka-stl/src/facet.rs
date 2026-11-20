use rikka_model_core::glam::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Facet {
    pub(crate) normal: Vec3,
    pub(crate) corners: [Vec3; 3],
    pub(crate) color: Option<[u8; 3]>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Solid {
    pub(crate) name: String,
    pub(crate) facets: Vec<Facet>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Document {
    pub(crate) solids: Vec<Solid>,
    pub(crate) object_color: Option<[u8; 4]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Palette {
    VisCam,
    Magics,
}

pub(crate) const VALID: u16 = 1 << 15;

impl Palette {
    pub(crate) fn decode(self, attribute: u16) -> Option<[u8; 3]> {
        let channel = |shift: u16| widen(((attribute >> shift) & 0x1f) as u8);
        match self {
            Self::VisCam => (attribute & VALID != 0).then(|| [channel(10), channel(5), channel(0)]),
            Self::Magics => (attribute & VALID == 0).then(|| [channel(0), channel(5), channel(10)]),
        }
    }
}

pub(crate) fn viscam(color: [u8; 3]) -> u16 {
    let narrow = |value: u8| u16::from(value >> 3);
    let [red, green, blue] = color;
    VALID | narrow(red) << 10 | narrow(green) << 5 | narrow(blue)
}

fn widen(five: u8) -> u8 {
    five << 3 | five >> 2
}
