use crate::buffer::glyph_buffer::Buffer;
use crate::buffer::glyph_info::{GlyphInfo, glyph_props};
use crate::face::font_context::FontContext;

pub(crate) mod lookup_flags {
    use rikka_text_font::LookupFlag;

    pub(crate) const RIGHT_TO_LEFT: u32 = LookupFlag::RIGHT_TO_LEFT as u32;
    pub(crate) const IGNORE_MARKS: u32 = LookupFlag::IGNORE_MARKS as u32;
    pub(crate) const IGNORE_FLAGS: u32 = (LookupFlag::IGNORE_BASE_GLYPHS
        | LookupFlag::IGNORE_LIGATURES
        | LookupFlag::IGNORE_MARKS) as u32;
    pub(crate) const USE_MARK_FILTERING_SET: u32 = LookupFlag::USE_MARK_FILTERING_SET as u32;
    pub(crate) const MARK_ATTACHMENT_TYPE: u32 = LookupFlag::MARK_ATTACHMENT_TYPE as u32;
}

#[inline]
pub(crate) fn check_glyph_property(font: &FontContext, info: &GlyphInfo, match_props: u32) -> bool {
    let props = u32::from(info.glyph_props);
    if props & match_props & lookup_flags::IGNORE_FLAGS != 0 {
        return false;
    }
    if props & u32::from(glyph_props::MARK) != 0 {
        if match_props & lookup_flags::USE_MARK_FILTERING_SET != 0 {
            return font.is_mark_in_set(info.codepoint, (match_props >> 16) as u16);
        }
        if match_props & lookup_flags::MARK_ATTACHMENT_TYPE != 0 {
            return match_props & lookup_flags::MARK_ATTACHMENT_TYPE
                == props & lookup_flags::MARK_ATTACHMENT_TYPE;
        }
    }
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Skip {
    No,
    Yes,
    Maybe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MatchResult {
    Match,
    NotMatch,
    Skip,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Matcher {
    pub(crate) lookup_props: u32,
    pub(crate) mask: u32,
    pub(crate) ignore_zwnj: bool,
    pub(crate) ignore_zwj: bool,
    pub(crate) ignore_hidden: bool,
    pub(crate) per_syllable: bool,
}

impl Matcher {
    #[inline]
    pub(crate) fn may_skip(&self, font: &FontContext, info: &GlyphInfo, lookup_props: u32) -> Skip {
        if !check_glyph_property(font, info, lookup_props) {
            return Skip::Yes;
        }
        if info.is_default_ignorable()
            && (self.ignore_zwnj || !info.is_zwnj())
            && (self.ignore_zwj || !info.is_zwj())
            && (self.ignore_hidden || !info.is_hidden())
        {
            return Skip::Maybe;
        }
        Skip::No
    }
}

pub(crate) trait GlyphPredicate {
    fn test(&self, glyph: u32, index: usize) -> Option<bool>;
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AnyGlyph;

impl GlyphPredicate for AnyGlyph {
    #[inline]
    fn test(&self, _: u32, _: usize) -> Option<bool> {
        None
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct EveryGlyph;

impl GlyphPredicate for EveryGlyph {
    #[inline]
    fn test(&self, _: u32, _: usize) -> Option<bool> {
        Some(true)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Matches<F>(pub(crate) F);

impl<F: Fn(u32, usize) -> bool> GlyphPredicate for Matches<F> {
    #[inline]
    fn test(&self, glyph: u32, index: usize) -> Option<bool> {
        Some((self.0)(glyph, index))
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Skippy {
    pub(crate) matcher: Matcher,
    pub(crate) lookup_props: u32,
    pub(crate) index: usize,
    end: usize,
    syllable: u8,
    data: usize,
}

impl Skippy {
    pub(crate) fn new(matcher: Matcher, buffer: &Buffer) -> Self {
        Self {
            lookup_props: matcher.lookup_props,
            matcher,
            index: 0,
            end: buffer.len,
            syllable: 0,
            data: 0,
        }
    }

    pub(crate) fn reset(&mut self, buffer: &Buffer, start: usize) {
        self.index = start;
        self.end = buffer.len;
        self.syllable = buffer.cur(0).syllable;
    }

    pub(crate) fn reset_back(&mut self, buffer: &Buffer, start: usize) {
        self.index = start;
        self.syllable = buffer.cur(0).syllable;
    }

    pub(crate) fn reset_fast(&mut self, start: usize) {
        self.index = start;
    }

    #[inline]
    pub(crate) fn may_skip(&self, font: &FontContext, info: &GlyphInfo) -> Skip {
        self.matcher.may_skip(font, info, self.lookup_props)
    }

    #[inline]
    fn match_at(
        &self,
        font: &FontContext,
        info: &GlyphInfo,
        predicate: &impl GlyphPredicate,
    ) -> MatchResult {
        let skip = self.may_skip(font, info);
        if skip == Skip::Yes {
            return MatchResult::Skip;
        }
        let may_match = if info.mask & self.matcher.mask == 0
            || (self.matcher.per_syllable && self.syllable != 0 && self.syllable != info.syllable)
        {
            Some(false)
        } else {
            predicate.test(info.codepoint, self.data)
        };
        match may_match {
            Some(true) => MatchResult::Match,
            None if skip == Skip::No => MatchResult::Match,
            _ if skip == Skip::No => MatchResult::NotMatch,
            _ => MatchResult::Skip,
        }
    }

    #[inline]
    pub(crate) fn matches_at(
        &self,
        font: &FontContext,
        info: &GlyphInfo,
        predicate: &impl GlyphPredicate,
    ) -> bool {
        self.match_at(font, info, predicate) == MatchResult::Match
    }

    pub(crate) fn next(
        &mut self,
        buffer: &Buffer,
        font: &FontContext,
        predicate: &impl GlyphPredicate,
        unsafe_to: &mut usize,
    ) -> bool {
        while self.index + 1 < self.end {
            self.index += 1;
            match self.match_at(font, &buffer.info[self.index], predicate) {
                MatchResult::Match => {
                    self.data += 1;
                    return true;
                }
                MatchResult::NotMatch => {
                    *unsafe_to = self.index + 1;
                    return false;
                }
                MatchResult::Skip => {}
            }
        }
        *unsafe_to = self.index + 1;
        false
    }

    pub(crate) fn prev(
        &mut self,
        buffer: &Buffer,
        font: &FontContext,
        predicate: &impl GlyphPredicate,
        unsafe_from: &mut usize,
    ) -> bool {
        while self.index > 0 {
            self.index -= 1;
            match self.match_at(font, &buffer.out_info()[self.index], predicate) {
                MatchResult::Match => {
                    self.data += 1;
                    return true;
                }
                MatchResult::NotMatch => {
                    *unsafe_from = self.index.max(1) - 1;
                    return false;
                }
                MatchResult::Skip => {}
            }
        }
        *unsafe_from = 0;
        false
    }
}
