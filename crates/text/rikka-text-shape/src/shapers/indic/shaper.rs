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
use crate::shapers::indic::final_reordering::final_reordering;
use crate::shapers::indic::initial_reordering::initial_reordering;
use crate::shapers::indic::machine::find_indic_syllables;
use crate::shapers::indic::plan::{BASIC_FEATURE_COUNT, FEATURES, IndicPlan};
use crate::shapers::indic::properties::indic_properties;
use crate::shapers::syllabic::syllables::{clear_syllables, mark_syllables_unsafe_to_break};
use crate::shapers::syllabic::vowel_constraints::insert_dotted_circles_in_invalid_vowels;

const UNDECOMPOSED: [u32; 4] = [0x0931, 0x09DC, 0x09DD, 0x0B94];
const BENGALI_YA: u32 = 0x09AF;
const BENGALI_NUKTA: u32 = 0x09BC;
const BENGALI_YYA: u32 = 0x09DF;

#[derive(Debug)]
pub(crate) struct IndicShaper;

pub(crate) static INDIC_SHAPER: IndicShaper = IndicShaper;

fn setup_syllables(_: &PlanCore, _: &FontContext, buffer: &mut Buffer) -> bool {
    find_indic_syllables(buffer);
    mark_syllables_unsafe_to_break(buffer);
    false
}

impl ComplexShaper for IndicShaper {
    fn collect_features(&self, planner: &mut Planner<'_>) {
        let map = &mut planner.map;
        map.add_gsub_pause(Some(setup_syllables));
        map.enable_feature(Tag::new(b"locl"), flags::PER_SYLLABLE, 1);
        map.enable_feature(Tag::new(b"ccmp"), flags::PER_SYLLABLE, 1);
        map.add_gsub_pause(Some(initial_reordering));
        for (tag, feature_flags) in &FEATURES[..BASIC_FEATURE_COUNT] {
            map.add_feature(Tag::new(tag), *feature_flags, 1);
            map.add_gsub_pause(None);
        }
        map.add_gsub_pause(Some(final_reordering));
        for (tag, feature_flags) in &FEATURES[BASIC_FEATURE_COUNT..] {
            map.add_feature(Tag::new(tag), *feature_flags, 1);
        }
    }

    fn override_features(&self, planner: &mut Planner<'_>) {
        planner.map.disable_feature(Tag::new(b"liga"));
        planner.map.add_gsub_pause(Some(clear_syllables));
    }

    fn create_data(&self, plan: &PlanCore, font: &FontContext) -> Option<ShaperData> {
        Some(Box::new(IndicPlan::new(plan, font)))
    }

    fn preprocess_text(&self, _: &PlanCore, _: &FontContext, buffer: &mut Buffer) {
        insert_dotted_circles_in_invalid_vowels(buffer);
    }

    fn normalization_mode(&self) -> NormalizationMode {
        NormalizationMode::ComposedDiacriticsNoShortCircuit
    }

    fn decompose(&self, _: &NormalizeContext<'_>, codepoint: u32) -> Option<(u32, u32)> {
        if UNDECOMPOSED.contains(&codepoint) {
            return None;
        }
        unicode_decompose(codepoint)
    }

    fn compose(&self, _: &NormalizeContext<'_>, first: u32, second: u32) -> Option<u32> {
        if first == BENGALI_YA && second == BENGALI_NUKTA {
            return Some(BENGALI_YYA);
        }
        compose_unless_mark(first, second)
    }

    fn setup_masks(&self, _: &PlanCore, _: &FontContext, buffer: &mut Buffer) {
        for info in &mut buffer.info[..buffer.len] {
            (info.shaper_category, info.shaper_auxiliary) = indic_properties(info.codepoint);
        }
    }

    fn zero_width_marks(&self) -> ZeroWidthMarks {
        ZeroWidthMarks::None
    }

    fn fallback_position(&self) -> bool {
        false
    }
}
