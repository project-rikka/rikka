use crate::buffer::glyph_info::GlyphInfo;
use crate::shapers::indic::properties::category;

#[inline]
pub(crate) const fn flag(category: u8) -> u64 {
    1 << category
}

pub(crate) const CONSONANTS: u64 = flag(category::C)
    | flag(category::CS)
    | flag(category::RA)
    | flag(category::CM)
    | flag(category::V)
    | flag(category::PLACEHOLDER)
    | flag(category::DOTTED_CIRCLE);

pub(crate) const JOINERS: u64 = flag(category::ZWJ) | flag(category::ZWNJ);

pub(crate) const MATRAS: u64 = flag(category::M) | flag(category::MPST);

#[inline]
pub(crate) fn is_one_of(info: &GlyphInfo, flags: u64) -> bool {
    !info.ligated() && flag(info.shaper_category & 63) & flags != 0
}

#[inline]
pub(crate) fn is_consonant(info: &GlyphInfo) -> bool {
    is_one_of(info, CONSONANTS)
}

#[inline]
pub(crate) fn is_joiner(info: &GlyphInfo) -> bool {
    is_one_of(info, JOINERS)
}

#[inline]
pub(crate) fn is_halant(info: &GlyphInfo) -> bool {
    is_one_of(info, flag(category::H))
}

#[inline]
pub(crate) fn in_flags(value: u8, flags: u64) -> bool {
    flag(value & 63) & flags != 0
}
