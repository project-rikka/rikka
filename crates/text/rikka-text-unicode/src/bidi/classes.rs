use crate::tables::enums::BidiClass;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ClassSet(u32);

impl ClassSet {
    pub(crate) const fn new(classes: &[BidiClass]) -> Self {
        let mut bits = 0;
        let mut index = 0;
        while index < classes.len() {
            bits |= 1 << classes[index] as u32;
            index += 1;
        }
        Self(bits)
    }

    #[inline]
    pub(crate) const fn has(self, class: BidiClass) -> bool {
        self.0 & (1 << class as u32) != 0
    }

    #[inline]
    pub(crate) const fn bit(class: BidiClass) -> u32 {
        1 << class as u32
    }

    #[inline]
    pub(crate) const fn intersects(self, bits: u32) -> bool {
        self.0 & bits != 0
    }
}

use BidiClass as B;

pub(crate) const ISOLATE_INITIATORS: ClassSet = ClassSet::new(&[
    B::LeftToRightIsolate,
    B::RightToLeftIsolate,
    B::FirstStrongIsolate,
]);
pub(crate) const ISOLATE_CONTROLS: ClassSet = ClassSet::new(&[
    B::LeftToRightIsolate,
    B::RightToLeftIsolate,
    B::FirstStrongIsolate,
    B::PopDirectionalIsolate,
]);
pub(crate) const REMOVED: ClassSet = ClassSet::new(&[
    B::LeftToRightEmbedding,
    B::RightToLeftEmbedding,
    B::LeftToRightOverride,
    B::RightToLeftOverride,
    B::PopDirectionalFormat,
    B::BoundaryNeutral,
]);
pub(crate) const NEUTRAL_OR_ISOLATE: ClassSet = ClassSet::new(&[
    B::ParagraphSeparator,
    B::SegmentSeparator,
    B::WhiteSpace,
    B::OtherNeutral,
    B::LeftToRightIsolate,
    B::RightToLeftIsolate,
    B::FirstStrongIsolate,
    B::PopDirectionalIsolate,
]);
pub(crate) const TRAILING_WHITESPACE: ClassSet = ClassSet::new(&[
    B::WhiteSpace,
    B::LeftToRightIsolate,
    B::RightToLeftIsolate,
    B::FirstStrongIsolate,
    B::PopDirectionalIsolate,
    B::LeftToRightEmbedding,
    B::RightToLeftEmbedding,
    B::LeftToRightOverride,
    B::RightToLeftOverride,
    B::PopDirectionalFormat,
    B::BoundaryNeutral,
]);
pub(crate) const SEPARATORS: ClassSet =
    ClassSet::new(&[B::SegmentSeparator, B::ParagraphSeparator]);
pub(crate) const NEEDS_RESOLUTION: ClassSet = ClassSet::new(&[
    B::RightToLeft,
    B::ArabicLetter,
    B::ArabicNumber,
    B::LeftToRightEmbedding,
    B::RightToLeftEmbedding,
    B::LeftToRightOverride,
    B::RightToLeftOverride,
    B::PopDirectionalFormat,
    B::LeftToRightIsolate,
    B::RightToLeftIsolate,
    B::FirstStrongIsolate,
    B::PopDirectionalIsolate,
]);

#[inline]
pub(crate) fn strong_direction(class: BidiClass) -> Option<BidiClass> {
    match class {
        B::LeftToRight => Some(B::LeftToRight),
        B::RightToLeft | B::EuropeanNumber | B::ArabicNumber => Some(B::RightToLeft),
        _ => None,
    }
}
