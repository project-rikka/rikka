use crate::tables::enums::{
    BidiClass, EastAsianWidth, GeneralCategory, GraphemeClusterBreak, IndicConjunctBreak,
    JoiningType, LineBreakClass, Script, WordBreak,
};
use crate::tables::properties::{
    BIDI_CLASS_MASK, BIDI_CLASS_SHIFT, BIDI_MIRRORED, BRACKET_TYPE_MASK, BRACKET_TYPE_SHIFT,
    COMBINING_CLASS_MASK, COMBINING_CLASS_SHIFT, DECOMPOSES, DEFAULT_IGNORABLE,
    EAST_ASIAN_WIDTH_MASK, EAST_ASIAN_WIDTH_SHIFT, EMOJI, EMOJI_COMPONENT, EMOJI_MODIFIER,
    EMOJI_MODIFIER_BASE, EMOJI_PRESENTATION, EXTENDED_PICTOGRAPHIC, GENERAL_CATEGORY_MASK,
    GENERAL_CATEGORY_SHIFT, GRAPHEME_CLUSTER_BREAK_MASK, GRAPHEME_CLUSTER_BREAK_SHIFT,
    HAS_SCRIPT_EXTENSIONS, INDIC_CONJUNCT_BREAK_MASK, INDIC_CONJUNCT_BREAK_SHIFT,
    JOINING_TYPE_MASK, JOINING_TYPE_SHIFT, LINE_BREAK_COLUMNS, LINE_BREAK_MASK, LINE_BREAK_SHIFT,
    NFC_MAYBE, NFC_NO, PROPERTIES, PROPERTIES_VALUES, SCRIPT_MASK, SCRIPT_SHIFT, WORD_BREAK_MASK,
    WORD_BREAK_SHIFT,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BracketType {
    Open,
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Properties(u64);

#[inline]
pub fn properties(character: char) -> Properties {
    Properties(PROPERTIES_VALUES[usize::from(PROPERTIES.get(character as u32))])
}

#[inline(always)]
pub(crate) fn line_break_column(codepoint: u32) -> u8 {
    LINE_BREAK_COLUMNS[usize::from(PROPERTIES.get(codepoint))]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QuickCheck {
    Yes,
    Maybe,
    No,
}

impl Properties {
    #[inline]
    fn field(self, shift: u32, mask: u64) -> u8 {
        ((self.0 >> shift) & mask) as u8
    }

    #[inline]
    fn flag(self, flag: u64) -> bool {
        self.0 & flag != 0
    }

    #[inline]
    pub fn general_category(self) -> GeneralCategory {
        GeneralCategory::from_bits(self.field(GENERAL_CATEGORY_SHIFT, GENERAL_CATEGORY_MASK))
    }

    #[inline]
    pub fn script(self) -> Script {
        Script::from_bits(self.field(SCRIPT_SHIFT, SCRIPT_MASK))
    }

    #[inline]
    pub fn bidi_class(self) -> BidiClass {
        BidiClass::from_bits(self.field(BIDI_CLASS_SHIFT, BIDI_CLASS_MASK))
    }

    #[inline]
    pub fn line_break_class(self) -> LineBreakClass {
        LineBreakClass::from_bits(self.field(LINE_BREAK_SHIFT, LINE_BREAK_MASK))
    }

    #[inline]
    pub fn grapheme_cluster_break(self) -> GraphemeClusterBreak {
        GraphemeClusterBreak::from_bits(
            self.field(GRAPHEME_CLUSTER_BREAK_SHIFT, GRAPHEME_CLUSTER_BREAK_MASK),
        )
    }

    #[inline]
    pub fn word_break(self) -> WordBreak {
        WordBreak::from_bits(self.field(WORD_BREAK_SHIFT, WORD_BREAK_MASK))
    }

    #[inline]
    pub fn east_asian_width(self) -> EastAsianWidth {
        EastAsianWidth::from_bits(self.field(EAST_ASIAN_WIDTH_SHIFT, EAST_ASIAN_WIDTH_MASK))
    }

    #[inline]
    pub fn indic_conjunct_break(self) -> IndicConjunctBreak {
        IndicConjunctBreak::from_bits(
            self.field(INDIC_CONJUNCT_BREAK_SHIFT, INDIC_CONJUNCT_BREAK_MASK),
        )
    }

    #[inline]
    pub fn joining_type(self) -> JoiningType {
        JoiningType::from_bits(self.field(JOINING_TYPE_SHIFT, JOINING_TYPE_MASK))
    }

    #[inline]
    pub fn bracket_type(self) -> Option<BracketType> {
        match self.field(BRACKET_TYPE_SHIFT, BRACKET_TYPE_MASK) {
            1 => Some(BracketType::Open),
            2 => Some(BracketType::Close),
            _ => None,
        }
    }

    #[inline]
    pub fn combining_class(self) -> u8 {
        self.field(COMBINING_CLASS_SHIFT, COMBINING_CLASS_MASK)
    }

    #[inline]
    pub fn is_extended_pictographic(self) -> bool {
        self.flag(EXTENDED_PICTOGRAPHIC)
    }

    #[inline]
    pub fn is_emoji(self) -> bool {
        self.flag(EMOJI)
    }

    #[inline]
    pub fn is_emoji_presentation(self) -> bool {
        self.flag(EMOJI_PRESENTATION)
    }

    #[inline]
    pub fn is_emoji_modifier(self) -> bool {
        self.flag(EMOJI_MODIFIER)
    }

    #[inline]
    pub fn is_emoji_modifier_base(self) -> bool {
        self.flag(EMOJI_MODIFIER_BASE)
    }

    #[inline]
    pub fn is_emoji_component(self) -> bool {
        self.flag(EMOJI_COMPONENT)
    }

    #[inline]
    pub fn is_regional_indicator(self) -> bool {
        self.grapheme_cluster_break() == GraphemeClusterBreak::RegionalIndicator
    }

    #[inline]
    pub fn is_default_ignorable(self) -> bool {
        self.flag(DEFAULT_IGNORABLE)
    }

    #[inline]
    pub fn is_bidi_mirrored(self) -> bool {
        self.flag(BIDI_MIRRORED)
    }

    #[inline]
    pub fn has_script_extensions(self) -> bool {
        self.flag(HAS_SCRIPT_EXTENSIONS)
    }

    #[inline]
    pub(crate) fn decomposes(self) -> bool {
        self.flag(DECOMPOSES)
    }

    #[inline]
    pub(crate) fn nfc_quick_check(self) -> QuickCheck {
        if self.flag(NFC_NO) {
            QuickCheck::No
        } else if self.flag(NFC_MAYBE) {
            QuickCheck::Maybe
        } else {
            QuickCheck::Yes
        }
    }
}

#[inline]
pub fn general_category(character: char) -> GeneralCategory {
    properties(character).general_category()
}

#[inline]
pub fn script(character: char) -> Script {
    properties(character).script()
}

#[inline]
pub fn bidi_class(character: char) -> BidiClass {
    properties(character).bidi_class()
}

#[inline]
pub fn line_break_class(character: char) -> LineBreakClass {
    properties(character).line_break_class()
}

#[inline]
pub fn grapheme_cluster_break(character: char) -> GraphemeClusterBreak {
    properties(character).grapheme_cluster_break()
}

#[inline]
pub fn word_break(character: char) -> WordBreak {
    properties(character).word_break()
}

#[inline]
pub fn east_asian_width(character: char) -> EastAsianWidth {
    properties(character).east_asian_width()
}

#[inline]
pub fn indic_conjunct_break(character: char) -> IndicConjunctBreak {
    properties(character).indic_conjunct_break()
}

#[inline]
pub fn joining_type(character: char) -> JoiningType {
    properties(character).joining_type()
}

#[inline]
pub fn combining_class(character: char) -> u8 {
    properties(character).combining_class()
}

#[inline]
pub fn is_default_ignorable(character: char) -> bool {
    properties(character).is_default_ignorable()
}

#[inline]
pub fn is_emoji(character: char) -> bool {
    properties(character).is_emoji()
}

#[inline]
pub fn is_emoji_presentation(character: char) -> bool {
    properties(character).is_emoji_presentation()
}

#[inline]
pub fn is_emoji_modifier(character: char) -> bool {
    properties(character).is_emoji_modifier()
}

#[inline]
pub fn is_emoji_modifier_base(character: char) -> bool {
    properties(character).is_emoji_modifier_base()
}

#[inline]
pub fn is_emoji_component(character: char) -> bool {
    properties(character).is_emoji_component()
}

#[inline]
pub fn is_extended_pictographic(character: char) -> bool {
    properties(character).is_extended_pictographic()
}

#[inline]
pub fn is_regional_indicator(character: char) -> bool {
    properties(character).is_regional_indicator()
}
