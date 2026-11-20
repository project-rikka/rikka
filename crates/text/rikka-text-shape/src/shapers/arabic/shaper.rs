use rikka_text_font::Tag;
use rikka_text_unicode::Script;

use crate::buffer::glyph_buffer::Buffer;
use crate::face::font_context::FontContext;
use crate::plan::map_builder::feature_flags as flags;
use crate::plan::planner::Planner;
use crate::plan::shape_plan::PlanCore;
use crate::shapers::arabic::fallback::ArabicFallback;
use crate::shapers::arabic::joining::{JOINING_FEATURES, JoiningPlan};
use crate::shapers::arabic::mark_reorder::reorder_modifier_marks;
use crate::shapers::arabic::stretch::{apply_stretches, record_stretches};
use crate::shapers::complex_shaper::{ComplexShaper, ShaperData};

#[derive(Debug)]
pub(crate) struct ArabicShaper;

pub(crate) static ARABIC_SHAPER: ArabicShaper = ArabicShaper;

#[derive(Debug)]
struct ArabicPlan {
    joining: JoiningPlan,
    has_stretch: bool,
    fallback: Option<ArabicFallback>,
}

fn is_syriac_feature(tag: Tag) -> bool {
    matches!(tag.to_bytes()[3], b'2' | b'3')
}

fn plan_data(plan: &PlanCore) -> Option<&ArabicPlan> {
    plan.shaper_data::<ArabicPlan>()
}

fn record_stch(plan: &PlanCore, _: &FontContext, buffer: &mut Buffer) -> bool {
    if plan_data(plan).is_some_and(|data| data.has_stretch) {
        record_stretches(buffer);
    }
    false
}

fn fallback_shape(plan: &PlanCore, font: &FontContext, buffer: &mut Buffer) -> bool {
    let Some(fallback) = plan_data(plan).and_then(|data| data.fallback.as_ref()) else {
        return false;
    };
    fallback.apply(font, buffer);
    true
}

impl ComplexShaper for ArabicShaper {
    fn collect_features(&self, planner: &mut Planner<'_>) {
        let arabic = planner.properties.script == Script::Arabic;
        let map = &mut planner.map;
        map.enable_feature(Tag::new(b"stch"), flags::NONE, 1);
        map.add_gsub_pause(Some(record_stch));
        map.enable_feature(Tag::new(b"ccmp"), flags::MANUAL_ZWJ, 1);
        map.enable_feature(Tag::new(b"locl"), flags::MANUAL_ZWJ, 1);
        map.add_gsub_pause(None);
        for tag in JOINING_FEATURES {
            let fallback = if arabic && !is_syriac_feature(tag) {
                flags::HAS_FALLBACK
            } else {
                flags::NONE
            };
            map.add_feature(tag, flags::MANUAL_ZWJ | fallback, 1);
            map.add_gsub_pause(None);
        }
        map.add_gsub_pause(None);
        map.enable_feature(
            Tag::new(b"rlig"),
            flags::MANUAL_ZWJ | flags::HAS_FALLBACK,
            1,
        );
        if arabic {
            map.add_gsub_pause(Some(fallback_shape));
        }
        map.enable_feature(Tag::new(b"calt"), flags::MANUAL_ZWJ, 1);
        if !map.has_feature(Tag::new(b"rclt")) {
            map.add_gsub_pause(None);
        }
        map.enable_feature(Tag::new(b"liga"), flags::MANUAL_ZWJ, 1);
        map.enable_feature(Tag::new(b"clig"), flags::MANUAL_ZWJ, 1);
        map.enable_feature(Tag::new(b"mset"), flags::MANUAL_ZWJ, 1);
    }

    fn create_data(&self, plan: &PlanCore, font: &FontContext) -> Option<ShaperData> {
        let map = &plan.map;
        let do_fallback = plan.properties.script == Script::Arabic
            && JOINING_FEATURES
                .iter()
                .all(|&tag| is_syriac_feature(tag) || map.needs_fallback(tag));
        Some(Box::new(ArabicPlan {
            joining: JoiningPlan::new(map),
            has_stretch: map.one_mask(Tag::new(b"stch")) != 0,
            fallback: if do_fallback {
                ArabicFallback::new(map, font)
            } else {
                None
            },
        }))
    }

    fn setup_masks(&self, plan: &PlanCore, _: &FontContext, buffer: &mut Buffer) {
        if let Some(data) = plan_data(plan) {
            data.joining.setup_masks(buffer, plan.properties.script);
        }
    }

    fn postprocess_glyphs(&self, _: &PlanCore, font: &FontContext, buffer: &mut Buffer) {
        apply_stretches(font, buffer);
    }

    fn reorder_marks(&self, _: &PlanCore, buffer: &mut Buffer, start: usize, end: usize) {
        reorder_modifier_marks(buffer, start, end);
    }
}
