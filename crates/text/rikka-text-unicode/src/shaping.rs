use crate::tables::enums::{IndicPositionalCategory, IndicSyllabicCategory, JoiningGroup};
use crate::tables::shaping::{
    INDIC_POSITIONAL_CATEGORY_MASK, INDIC_POSITIONAL_CATEGORY_SHIFT, INDIC_SYLLABIC_CATEGORY_MASK,
    INDIC_SYLLABIC_CATEGORY_SHIFT, JOINING_GROUP_MASK, JOINING_GROUP_SHIFT, SHAPING,
    SHAPING_VALUES,
};

#[inline]
fn field(character: char, shift: u32, mask: u32) -> u8 {
    ((SHAPING_VALUES[usize::from(SHAPING.get(character as u32))] >> shift) & mask) as u8
}

#[inline]
pub fn indic_syllabic_category(character: char) -> IndicSyllabicCategory {
    IndicSyllabicCategory::from_bits(field(
        character,
        INDIC_SYLLABIC_CATEGORY_SHIFT,
        INDIC_SYLLABIC_CATEGORY_MASK,
    ))
}

#[inline]
pub fn indic_positional_category(character: char) -> IndicPositionalCategory {
    IndicPositionalCategory::from_bits(field(
        character,
        INDIC_POSITIONAL_CATEGORY_SHIFT,
        INDIC_POSITIONAL_CATEGORY_MASK,
    ))
}

#[inline]
pub fn joining_group(character: char) -> JoiningGroup {
    JoiningGroup::from_bits(field(character, JOINING_GROUP_SHIFT, JOINING_GROUP_MASK))
}
