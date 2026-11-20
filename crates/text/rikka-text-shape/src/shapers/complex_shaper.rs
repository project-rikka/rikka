use std::any::Any;
use std::fmt::Debug;

use rikka_text_font::Tag;

use crate::buffer::glyph_buffer::Buffer;
use crate::face::font_context::FontContext;
use crate::pipeline::normalize::NormalizeContext;
use crate::plan::planner::Planner;
use crate::plan::shape_plan::PlanCore;
use crate::unicode::to_char;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NormalizationMode {
    None,
    ComposedDiacritics,
    ComposedDiacriticsNoShortCircuit,
    Auto,
}

impl NormalizationMode {
    pub(crate) fn resolved(self) -> Self {
        match self {
            Self::Auto => Self::ComposedDiacritics,
            mode => mode,
        }
    }

    pub(crate) fn always_short_circuits(self) -> bool {
        self == Self::None
    }

    pub(crate) fn might_short_circuit(self) -> bool {
        matches!(self, Self::None | Self::ComposedDiacritics | Self::Auto)
    }

    pub(crate) fn recomposes(self) -> bool {
        matches!(
            self,
            Self::ComposedDiacritics | Self::ComposedDiacriticsNoShortCircuit
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ZeroWidthMarks {
    None,
    ByGdefEarly,
    ByGdefLate,
}

impl ZeroWidthMarks {
    pub(crate) fn zeroes_before_positioning(self) -> bool {
        self == Self::ByGdefEarly
    }

    pub(crate) fn zeroes_after_positioning(self) -> bool {
        self == Self::ByGdefLate
    }
}

pub(crate) type ShaperData = Box<dyn Any + Send + Sync>;

pub(crate) trait ComplexShaper: Debug + Sync {
    fn collect_features(&self, _planner: &mut Planner<'_>) {}

    fn override_features(&self, _planner: &mut Planner<'_>) {}

    fn create_data(&self, _plan: &PlanCore, _font: &FontContext) -> Option<ShaperData> {
        None
    }

    fn preprocess_text(&self, _plan: &PlanCore, _font: &FontContext, _buffer: &mut Buffer) {}

    fn postprocess_glyphs(&self, _plan: &PlanCore, _font: &FontContext, _buffer: &mut Buffer) {}

    fn normalization_mode(&self) -> NormalizationMode {
        NormalizationMode::Auto
    }

    fn decompose(&self, _context: &NormalizeContext<'_>, codepoint: u32) -> Option<(u32, u32)> {
        unicode_decompose(codepoint)
    }

    fn compose(&self, _context: &NormalizeContext<'_>, first: u32, second: u32) -> Option<u32> {
        unicode_compose(first, second)
    }

    fn setup_masks(&self, _plan: &PlanCore, _font: &FontContext, _buffer: &mut Buffer) {}

    fn gpos_tag(&self) -> Option<Tag> {
        None
    }

    fn reorder_marks(&self, _plan: &PlanCore, _buffer: &mut Buffer, _start: usize, _end: usize) {}

    fn zero_width_marks(&self) -> ZeroWidthMarks {
        ZeroWidthMarks::ByGdefLate
    }

    fn is_script_neutral(&self) -> bool {
        false
    }

    fn fallback_position(&self) -> bool {
        true
    }
}

pub(crate) fn unicode_decompose(codepoint: u32) -> Option<(u32, u32)> {
    let (first, second) = rikka_text_unicode::decompose(to_char(codepoint))?;
    Some((first as u32, second.map_or(0, |second| second as u32)))
}

pub(crate) fn unicode_compose(first: u32, second: u32) -> Option<u32> {
    rikka_text_unicode::compose(to_char(first), to_char(second)).map(|composed| composed as u32)
}

pub(crate) fn compose_unless_mark(first: u32, second: u32) -> Option<u32> {
    if rikka_text_unicode::general_category(to_char(first)).is_mark() {
        return None;
    }
    unicode_compose(first, second)
}
