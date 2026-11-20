use crate::buffer::glyph_buffer::Buffer;
use crate::buffer::glyph_info::glyph_props;
use crate::face::font_context::FontContext;
use crate::layout::dispatch::apply_lookup_here;
use crate::layout::matching::{Matcher, Skippy};
use crate::plan::feature_map::TableKind;

pub(crate) const MAX_NESTING_LEVEL: usize = 64;
pub(crate) const MAX_CONTEXT_LENGTH: usize = 64;

#[derive(Debug)]
pub(crate) struct ApplyContext<'b> {
    pub(crate) table: TableKind,
    pub(crate) font: &'b FontContext,
    pub(crate) buffer: &'b mut Buffer,
    pub(crate) lookup_mask: u32,
    pub(crate) lookup_props: u32,
    pub(crate) lookup_index: u16,
    pub(crate) auto_zwnj: bool,
    pub(crate) auto_zwj: bool,
    pub(crate) random: bool,
    pub(crate) per_syllable: bool,
    pub(crate) random_state: u32,
    pub(crate) nesting_level_left: usize,
    pub(crate) last_base: Option<usize>,
    pub(crate) last_base_until: usize,
    pub(crate) matcher: Matcher,
    pub(crate) context_matcher: Matcher,
}

impl<'b> ApplyContext<'b> {
    pub(crate) fn new(table: TableKind, font: &'b FontContext, buffer: &'b mut Buffer) -> Self {
        Self {
            table,
            font,
            buffer,
            lookup_mask: 1,
            lookup_props: 0,
            lookup_index: u16::MAX,
            auto_zwnj: true,
            auto_zwj: true,
            random: false,
            per_syllable: false,
            random_state: 1,
            nesting_level_left: MAX_NESTING_LEVEL,
            last_base: None,
            last_base_until: 0,
            matcher: Matcher::default(),
            context_matcher: Matcher::default(),
        }
    }

    pub(crate) fn set_lookup_mask(&mut self, mask: u32) {
        self.lookup_mask = mask;
        self.last_base = None;
        self.last_base_until = 0;
    }

    pub(crate) fn update_matchers(&mut self) {
        let gpos = self.table == TableKind::Gpos;
        let gsub = self.table == TableKind::Gsub;
        self.matcher = Matcher {
            lookup_props: self.lookup_props,
            mask: self.lookup_mask,
            ignore_zwnj: gpos,
            ignore_zwj: self.auto_zwj,
            ignore_hidden: gpos,
            per_syllable: gsub && self.per_syllable,
        };
        self.context_matcher = Matcher {
            lookup_props: self.lookup_props,
            mask: u32::MAX,
            ignore_zwnj: gpos || self.auto_zwnj,
            ignore_zwj: true,
            ignore_hidden: gpos,
            per_syllable: gsub && self.per_syllable,
        };
    }

    #[inline]
    pub(crate) fn input_iterator(&self) -> Skippy {
        Skippy::new(self.matcher, self.buffer)
    }

    #[inline]
    pub(crate) fn context_iterator(&self) -> Skippy {
        Skippy::new(self.context_matcher, self.buffer)
    }

    pub(crate) fn random_number(&mut self) -> u32 {
        self.random_state = self.random_state.wrapping_mul(48271) % (i32::MAX as u32);
        self.random_state
    }

    pub(crate) fn recurse(&mut self, lookup_index: u16) -> bool {
        if self.nesting_level_left == 0 {
            return false;
        }
        self.buffer.max_ops -= 1;
        if self.buffer.max_ops < 0 {
            self.buffer.successful = false;
            return false;
        }
        self.nesting_level_left -= 1;
        let saved_props = self.lookup_props;
        let saved_index = self.lookup_index;
        self.lookup_index = lookup_index;
        let applied = apply_lookup_here(self, lookup_index);
        self.lookup_props = saved_props;
        self.lookup_index = saved_index;
        self.update_matchers();
        self.nesting_level_left += 1;
        applied
    }

    fn set_glyph_class(&mut self, glyph: u32, class_guess: u16, ligature: bool, component: bool) {
        self.buffer.digest.add(glyph);
        let font = self.font;
        let current = self.buffer.cur_mut(0);
        let mut props = current.glyph_props | glyph_props::SUBSTITUTED;
        if ligature {
            props |= glyph_props::LIGATED;
            props &= !glyph_props::MULTIPLIED;
        }
        if component {
            props |= glyph_props::MULTIPLIED;
        }
        current.glyph_props = if font.has_glyph_classes() {
            (props & glyph_props::PRESERVE) | font.glyph_props(glyph)
        } else if class_guess != 0 {
            (props & glyph_props::PRESERVE) | class_guess
        } else {
            props
        };
    }

    pub(crate) fn replace_glyph(&mut self, glyph: u32) {
        self.set_glyph_class(glyph, 0, false, false);
        self.buffer.replace_glyph(glyph);
    }

    pub(crate) fn replace_glyph_inplace(&mut self, glyph: u32) {
        self.set_glyph_class(glyph, 0, false, false);
        self.buffer.cur_mut(0).codepoint = glyph;
    }

    pub(crate) fn replace_glyph_with_ligature(&mut self, glyph: u32, class_guess: u16) {
        self.set_glyph_class(glyph, class_guess, true, false);
        self.buffer.replace_glyph(glyph);
    }

    pub(crate) fn output_glyph_for_component(&mut self, glyph: u32, class_guess: u16) {
        self.set_glyph_class(glyph, class_guess, false, true);
        self.buffer.output_glyph(glyph);
    }
}
