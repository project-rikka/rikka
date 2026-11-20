use rikka_text_font::Tag;

use crate::buffer::glyph_buffer::Buffer;
use crate::face::font_context::FontContext;
use crate::pipeline::normalize::NormalizeContext;
use crate::plan::map_builder::feature_flags as flags;
use crate::plan::planner::Planner;
use crate::plan::shape_plan::PlanCore;
use crate::shapers::complex_shaper::{
    ComplexShaper, NormalizationMode, ShaperData, ZeroWidthMarks, compose_unless_mark,
    unicode_decompose,
};
use crate::shapers::indic::properties::indic_properties;
use crate::shapers::khmer::machine::find_khmer_syllables;
use crate::shapers::khmer::reorder::reorder;
use crate::shapers::syllabic::syllables::{clear_syllables, mark_syllables_unsafe_to_break};

const BASIC_FEATURES: [&[u8; 4]; 5] = [b"pref", b"blwf", b"abvf", b"pstf", b"cfar"];
const OTHER_FEATURES: [&[u8; 4]; 4] = [b"pres", b"abvs", b"blws", b"psts"];
const SPLIT_VOWEL_PREFIX: u32 = 0x17C1;

#[derive(Debug)]
pub(crate) struct KhmerShaper;

pub(crate) static KHMER_SHAPER: KhmerShaper = KhmerShaper;

#[derive(Debug)]
pub(crate) struct KhmerPlan {
    pub(crate) pre_base: u32,
    pub(crate) below: u32,
    pub(crate) above: u32,
    pub(crate) post: u32,
    pub(crate) conjunct_after_ro: u32,
}

fn setup_syllables(_: &PlanCore, _: &FontContext, buffer: &mut Buffer) -> bool {
    find_khmer_syllables(buffer);
    mark_syllables_unsafe_to_break(buffer);
    false
}

impl ComplexShaper for KhmerShaper {
    fn collect_features(&self, planner: &mut Planner<'_>) {
        let map = &mut planner.map;
        map.add_gsub_pause(Some(setup_syllables));
        map.add_gsub_pause(Some(reorder));
        map.enable_feature(Tag::new(b"locl"), flags::PER_SYLLABLE, 1);
        map.enable_feature(Tag::new(b"ccmp"), flags::PER_SYLLABLE, 1);
        for tag in BASIC_FEATURES {
            map.add_feature(
                Tag::new(tag),
                flags::MANUAL_JOINERS | flags::PER_SYLLABLE,
                1,
            );
        }
        map.add_gsub_pause(Some(clear_syllables));
        for tag in OTHER_FEATURES {
            map.add_feature(Tag::new(tag), flags::GLOBAL_MANUAL_JOINERS, 1);
        }
    }

    fn override_features(&self, planner: &mut Planner<'_>) {
        planner
            .map
            .enable_feature(Tag::new(b"clig"), flags::NONE, 1);
        planner.map.disable_feature(Tag::new(b"liga"));
    }

    fn create_data(&self, plan: &PlanCore, _: &FontContext) -> Option<ShaperData> {
        let mask = |tag| plan.map.one_mask(Tag::new(tag));
        Some(Box::new(KhmerPlan {
            pre_base: mask(b"pref"),
            below: mask(b"blwf"),
            above: mask(b"abvf"),
            post: mask(b"pstf"),
            conjunct_after_ro: mask(b"cfar"),
        }))
    }

    fn normalization_mode(&self) -> NormalizationMode {
        NormalizationMode::ComposedDiacriticsNoShortCircuit
    }

    fn decompose(&self, _: &NormalizeContext<'_>, codepoint: u32) -> Option<(u32, u32)> {
        match codepoint {
            0x17BE | 0x17BF | 0x17C0 | 0x17C4 | 0x17C5 => Some((SPLIT_VOWEL_PREFIX, codepoint)),
            _ => unicode_decompose(codepoint),
        }
    }

    fn compose(&self, _: &NormalizeContext<'_>, first: u32, second: u32) -> Option<u32> {
        compose_unless_mark(first, second)
    }

    fn setup_masks(&self, _: &PlanCore, _: &FontContext, buffer: &mut Buffer) {
        for info in &mut buffer.info[..buffer.len] {
            info.shaper_category = indic_properties(info.codepoint).0;
        }
    }

    fn zero_width_marks(&self) -> ZeroWidthMarks {
        ZeroWidthMarks::None
    }

    fn fallback_position(&self) -> bool {
        false
    }
}
