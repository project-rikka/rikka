use rikka_text_font::{Synthesis, Tag};

use crate::face::FaceId;
use crate::style::{FontStyle, Weight, Width};

const OPTICAL_SIZE_AXIS: Tag = Tag::new(b"opsz");
const MAX_SETTINGS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct StyleRequest {
    pub(crate) weight: Weight,
    pub(crate) width: Width,
    pub(crate) style: FontStyle,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Variations {
    settings: [(Tag, u32); MAX_SETTINGS],
    len: u8,
    optical_size: bool,
}

impl Variations {
    pub(crate) fn set(&mut self, tag: Tag, value: f32) {
        if usize::from(self.len) < MAX_SETTINGS {
            self.settings[usize::from(self.len)] = (tag, value.to_bits());
            self.len += 1;
        }
    }

    pub(crate) fn enable_optical_size(&mut self) {
        self.optical_size = true;
    }

    fn settings(&self, font_size: f32) -> impl Iterator<Item = (Tag, f32)> + use<> {
        let fixed = self.settings;
        let optical_size = self.optical_size.then_some((OPTICAL_SIZE_AXIS, font_size));
        fixed
            .into_iter()
            .take(usize::from(self.len))
            .map(|(tag, bits)| (tag, f32::from_bits(bits)))
            .chain(optical_size)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ChainId(pub(crate) u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FaceMatch {
    pub(crate) face: FaceId,
    pub(crate) synthesis: Synthesis,
    pub(crate) variations: Variations,
    pub(crate) request: StyleRequest,
    pub(crate) chain: ChainId,
}

impl FaceMatch {
    #[inline]
    pub fn face(&self) -> FaceId {
        self.face
    }

    #[inline]
    pub fn synthesis(&self) -> Synthesis {
        self.synthesis
    }

    pub fn weight(&self) -> Weight {
        self.request.weight
    }

    pub fn width(&self) -> Width {
        self.request.width
    }

    pub fn style(&self) -> FontStyle {
        self.request.style
    }

    pub fn variations(&self, font_size: f32) -> impl Iterator<Item = (Tag, f32)> + use<> {
        self.variations.settings(font_size)
    }
}
