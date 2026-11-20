use rikka_text_font::GlyphId;
use rikka_text_unicode::GeneralCategory;

use crate::buffer::scratch_flags;
use crate::unicode::combining_class::modified_combining_class;
use crate::unicode::ignorables::is_default_ignorable;
use crate::unicode::space_fallback::SpaceKind;
use crate::unicode::to_char;

pub(crate) mod glyph_flag {
    pub(crate) const UNSAFE_TO_BREAK: u32 = 0x01;
    pub(crate) const UNSAFE_TO_CONCAT: u32 = 0x02;
    pub(crate) const SAFE_TO_INSERT_TATWEEL: u32 = 0x04;
    pub(crate) const DEFINED: u32 = 0x07;
}

pub(crate) mod unicode_props {
    pub(crate) const GENERAL_CATEGORY: u16 = 0x001F;
    pub(crate) const IGNORABLE: u16 = 0x0020;
    pub(crate) const HIDDEN: u16 = 0x0040;
    pub(crate) const CONTINUATION: u16 = 0x0080;
    pub(crate) const CF_ZWJ: u16 = 0x0100;
    pub(crate) const CF_ZWNJ: u16 = 0x0200;
    pub(crate) const CF_VARIATION_SELECTOR: u16 = 0x0400;
}

pub(crate) mod glyph_props {
    pub(crate) const BASE_GLYPH: u16 = 0x02;
    pub(crate) const LIGATURE: u16 = 0x04;
    pub(crate) const MARK: u16 = 0x08;
    pub(crate) const SUBSTITUTED: u16 = 0x10;
    pub(crate) const LIGATED: u16 = 0x20;
    pub(crate) const MULTIPLIED: u16 = 0x40;
    pub(crate) const PRESERVE: u16 = SUBSTITUTED | LIGATED | MULTIPLIED;
}

const IS_LIGATURE_BASE: u8 = 0x10;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlyphInfo {
    pub(crate) codepoint: u32,
    pub(crate) mask: u32,
    pub(crate) cluster: u32,
    pub(crate) glyph_props: u16,
    pub(crate) unicode_props: u16,
    pub(crate) lig_props: u8,
    pub(crate) syllable: u8,
    pub(crate) shaper_category: u8,
    pub(crate) shaper_auxiliary: u8,
    pub(crate) nominal_glyph: u16,
}

impl GlyphInfo {
    #[inline]
    pub fn glyph(&self) -> GlyphId {
        GlyphId(self.codepoint as u16)
    }

    #[inline]
    pub fn cluster(&self) -> u32 {
        self.cluster
    }

    #[inline]
    pub fn unsafe_to_break(&self) -> bool {
        self.mask & glyph_flag::UNSAFE_TO_BREAK != 0
    }

    #[inline]
    pub fn unsafe_to_concat(&self) -> bool {
        self.mask & glyph_flag::UNSAFE_TO_CONCAT != 0
    }

    #[inline]
    pub fn safe_to_insert_tatweel(&self) -> bool {
        self.mask & glyph_flag::SAFE_TO_INSERT_TATWEEL != 0
    }

    pub(crate) fn new(codepoint: u32, cluster: u32) -> Self {
        Self {
            codepoint,
            cluster,
            ..Self::default()
        }
    }

    pub(crate) fn init_unicode_props(&mut self, scratch: &mut u32) {
        let codepoint = self.codepoint;
        let properties = rikka_text_unicode::properties(to_char(codepoint));
        let category = properties.general_category();
        let mut props = category as u16;
        if codepoint >= 0x80 {
            if is_default_ignorable(codepoint) {
                props |= unicode_props::IGNORABLE;
                *scratch |= scratch_flags::HAS_DEFAULT_IGNORABLES;
                match codepoint {
                    0x200C => props |= unicode_props::CF_ZWNJ,
                    0x200D => props |= unicode_props::CF_ZWJ,
                    0x180B..=0x180D | 0x180F | 0xE0020..=0xE007F => {
                        props |= unicode_props::HIDDEN;
                    }
                    0x034F => {
                        props |= unicode_props::HIDDEN;
                        *scratch |= scratch_flags::HAS_CGJ;
                    }
                    _ => {}
                }
            }
            if category.is_mark() {
                *scratch |= scratch_flags::HAS_CONTINUATIONS;
                props |= unicode_props::CONTINUATION;
                props |= u16::from(modified_combining_class(
                    codepoint,
                    properties.combining_class(),
                )) << 8;
            }
        }
        self.unicode_props = props;
    }

    #[inline]
    pub(crate) fn general_category(&self) -> GeneralCategory {
        GeneralCategory::from_bits((self.unicode_props & unicode_props::GENERAL_CATEGORY) as u8)
    }

    #[inline]
    pub(crate) fn set_general_category(&mut self, category: GeneralCategory) {
        self.unicode_props =
            category as u16 | (self.unicode_props & (0xFF & !unicode_props::GENERAL_CATEGORY));
    }

    #[inline]
    pub(crate) fn is_unicode_mark(&self) -> bool {
        self.general_category().is_mark()
    }

    #[inline]
    pub(crate) fn modified_combining_class(&self) -> u8 {
        if self.is_unicode_mark() {
            (self.unicode_props >> 8) as u8
        } else {
            0
        }
    }

    #[inline]
    pub(crate) fn set_modified_combining_class(&mut self, class: u8) {
        if self.is_unicode_mark() {
            self.unicode_props = (u16::from(class) << 8) | (self.unicode_props & 0xFF);
        }
    }

    #[inline]
    pub(crate) fn is_unicode_space(&self) -> bool {
        self.general_category() == GeneralCategory::SpaceSeparator
    }

    #[inline]
    pub(crate) fn space_fallback(&self) -> SpaceKind {
        if self.is_unicode_space() {
            SpaceKind::from_bits((self.unicode_props >> 8) as u8)
        } else {
            SpaceKind::NotSpace
        }
    }

    #[inline]
    pub(crate) fn set_space_fallback(&mut self, kind: SpaceKind) {
        if self.is_unicode_space() {
            self.unicode_props = ((kind as u16) << 8) | (self.unicode_props & 0xFF);
        }
    }

    #[inline]
    pub(crate) fn mark_as_variation_selector(&mut self) {
        self.set_general_category(GeneralCategory::Format);
        self.unicode_props |= unicode_props::CF_VARIATION_SELECTOR;
    }

    #[inline]
    pub(crate) fn is_default_ignorable(&self) -> bool {
        self.unicode_props & unicode_props::IGNORABLE != 0 && !self.substituted()
    }

    #[inline]
    pub(crate) fn is_hidden(&self) -> bool {
        self.unicode_props & unicode_props::HIDDEN != 0
    }

    #[inline]
    pub(crate) fn unhide(&mut self) {
        self.unicode_props &= !unicode_props::HIDDEN;
    }

    #[inline]
    pub(crate) fn is_continuation(&self) -> bool {
        self.unicode_props & unicode_props::CONTINUATION != 0
    }

    #[inline]
    pub(crate) fn set_continuation(&mut self, scratch: &mut u32) {
        *scratch |= scratch_flags::HAS_CONTINUATIONS;
        self.unicode_props |= unicode_props::CONTINUATION;
    }

    #[inline]
    pub(crate) fn clear_continuation(&mut self) {
        self.unicode_props &= !unicode_props::CONTINUATION;
    }

    #[inline]
    pub(crate) fn is_unicode_format(&self) -> bool {
        self.general_category() == GeneralCategory::Format
    }

    #[inline]
    pub(crate) fn is_zwnj(&self) -> bool {
        self.is_unicode_format() && self.unicode_props & unicode_props::CF_ZWNJ != 0
    }

    #[inline]
    pub(crate) fn is_zwj(&self) -> bool {
        self.is_unicode_format() && self.unicode_props & unicode_props::CF_ZWJ != 0
    }

    #[inline]
    pub(crate) fn set_lig_props_for_ligature(&mut self, lig_id: u8, component_count: u8) {
        self.lig_props = (lig_id << 5) | IS_LIGATURE_BASE | (component_count & 0x0F);
    }

    #[inline]
    pub(crate) fn set_lig_props_for_mark(&mut self, lig_id: u8, component: u8) {
        self.lig_props = (lig_id << 5) | (component & 0x0F);
    }

    #[inline]
    pub(crate) fn set_lig_props_for_component(&mut self, component: u8) {
        self.set_lig_props_for_mark(0, component);
    }

    #[inline]
    pub(crate) fn lig_id(&self) -> u8 {
        self.lig_props >> 5
    }

    #[inline]
    fn ligated_internal(&self) -> bool {
        self.lig_props & IS_LIGATURE_BASE != 0
    }

    #[inline]
    pub(crate) fn lig_comp(&self) -> u8 {
        if self.ligated_internal() {
            0
        } else {
            self.lig_props & 0x0F
        }
    }

    #[inline]
    pub(crate) fn lig_num_comps(&self) -> u8 {
        if self.glyph_props & glyph_props::LIGATURE != 0 && self.ligated_internal() {
            self.lig_props & 0x0F
        } else {
            1
        }
    }

    #[inline]
    pub(crate) fn is_base_glyph(&self) -> bool {
        self.glyph_props & glyph_props::BASE_GLYPH != 0
    }

    #[inline]
    pub(crate) fn is_ligature(&self) -> bool {
        self.glyph_props & glyph_props::LIGATURE != 0
    }

    #[inline]
    pub(crate) fn is_mark(&self) -> bool {
        self.glyph_props & glyph_props::MARK != 0
    }

    #[inline]
    pub(crate) fn substituted(&self) -> bool {
        self.glyph_props & glyph_props::SUBSTITUTED != 0
    }

    #[inline]
    pub(crate) fn ligated(&self) -> bool {
        self.glyph_props & glyph_props::LIGATED != 0
    }

    #[inline]
    pub(crate) fn multiplied(&self) -> bool {
        self.glyph_props & glyph_props::MULTIPLIED != 0
    }

    #[inline]
    pub(crate) fn ligated_and_didnt_multiply(&self) -> bool {
        self.ligated() && !self.multiplied()
    }

    #[inline]
    pub(crate) fn clear_ligated_and_multiplied(&mut self) {
        self.glyph_props &= !(glyph_props::LIGATED | glyph_props::MULTIPLIED);
    }

    #[inline]
    pub(crate) fn clear_substituted(&mut self) {
        self.glyph_props &= !glyph_props::SUBSTITUTED;
    }
}
