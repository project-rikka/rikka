use rikka_text_font::Tag;

use crate::buffer::glyph_buffer::Buffer;
use crate::face::font_context::FontContext;
use crate::plan::map_builder::feature_flags as flags;
use crate::plan::planner::Planner;
use crate::plan::shape_plan::PlanCore;
use crate::shapers::complex_shaper::{ComplexShaper, NormalizationMode, ZeroWidthMarks};
use crate::shapers::indic::properties::indic_properties;
use crate::shapers::myanmar::machine::find_myanmar_syllables;
use crate::shapers::myanmar::reorder::reorder;
use crate::shapers::syllabic::syllables::{clear_syllables, mark_syllables_unsafe_to_break};

const BASIC_FEATURES: [&[u8; 4]; 4] = [b"rphf", b"pref", b"blwf", b"pstf"];
const OTHER_FEATURES: [&[u8; 4]; 4] = [b"pres", b"abvs", b"blws", b"psts"];

#[derive(Debug)]
pub(crate) struct MyanmarShaper;

pub(crate) static MYANMAR_SHAPER: MyanmarShaper = MyanmarShaper;

fn setup_syllables(_: &PlanCore, _: &FontContext, buffer: &mut Buffer) -> bool {
    find_myanmar_syllables(buffer);
    mark_syllables_unsafe_to_break(buffer);
    false
}

impl ComplexShaper for MyanmarShaper {
    fn collect_features(&self, planner: &mut Planner<'_>) {
        let map = &mut planner.map;
        map.add_gsub_pause(Some(setup_syllables));
        map.enable_feature(Tag::new(b"locl"), flags::PER_SYLLABLE, 1);
        map.enable_feature(Tag::new(b"ccmp"), flags::PER_SYLLABLE, 1);
        map.add_gsub_pause(Some(reorder));
        for tag in BASIC_FEATURES {
            map.enable_feature(Tag::new(tag), flags::MANUAL_ZWJ | flags::PER_SYLLABLE, 1);
            map.add_gsub_pause(None);
        }
        map.add_gsub_pause(Some(clear_syllables));
        for tag in OTHER_FEATURES {
            map.enable_feature(Tag::new(tag), flags::MANUAL_ZWJ, 1);
        }
    }

    fn normalization_mode(&self) -> NormalizationMode {
        NormalizationMode::ComposedDiacriticsNoShortCircuit
    }

    fn setup_masks(&self, _: &PlanCore, _: &FontContext, buffer: &mut Buffer) {
        for info in &mut buffer.info[..buffer.len] {
            info.shaper_category = indic_properties(info.codepoint).0;
        }
    }

    fn zero_width_marks(&self) -> ZeroWidthMarks {
        ZeroWidthMarks::ByGdefEarly
    }

    fn fallback_position(&self) -> bool {
        false
    }
}
