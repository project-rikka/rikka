use rikka_text_font::Tag;
use rikka_text_unicode::Script;

use crate::buffer::glyph_buffer::Buffer;
use crate::face::font_context::FontContext;
use crate::pipeline::normalize::NormalizeContext;
use crate::plan::map_builder::feature_flags as flags;
use crate::plan::planner::Planner;
use crate::plan::shape_plan::PlanCore;
use crate::shapers::arabic::joining::JoiningPlan;
use crate::shapers::complex_shaper::{
    ComplexShaper, NormalizationMode, ShaperData, ZeroWidthMarks, compose_unless_mark,
};
use crate::shapers::syllabic::syllables::{
    clear_syllables, for_each_syllable, mark_syllables_unsafe_to_break,
};
use crate::shapers::syllabic::vowel_constraints::insert_dotted_circles_in_invalid_vowels;
use crate::shapers::universal::categories::{category as cat, use_category};
use crate::shapers::universal::machine::find_universal_syllables;
use crate::shapers::universal::reorder::reorder;
use crate::shapers::universal::topography::{TOPOGRAPHICAL_FEATURES, Topography};

const BASIC_FEATURES: [&[u8; 4]; 7] = [
    b"rkrf", b"abvf", b"blwf", b"half", b"pstf", b"vatu", b"cjct",
];
const OTHER_FEATURES: [&[u8; 4]; 5] = [b"abvs", b"blws", b"haln", b"pres", b"psts"];
const REPHA_LENGTH: usize = 3;

#[derive(Debug)]
pub(crate) struct UniversalShaper;

pub(crate) static UNIVERSAL_SHAPER: UniversalShaper = UniversalShaper;

#[derive(Debug)]
struct UniversalPlan {
    rphf: u32,
    joining: Option<JoiningPlan>,
    topography: Option<Topography>,
}

fn has_arabic_joining(script: Script) -> bool {
    matches!(
        script,
        Script::Adlam
            | Script::Chorasmian
            | Script::HanifiRohingya
            | Script::Mandaic
            | Script::Manichaean
            | Script::Mongolian
            | Script::Nko
            | Script::OldUyghur
            | Script::PhagsPa
            | Script::PsalterPahlavi
            | Script::Sogdian
    )
}

fn plan_data(plan: &PlanCore) -> Option<&UniversalPlan> {
    plan.shaper_data::<UniversalPlan>()
}

fn setup_syllables(plan: &PlanCore, _: &FontContext, buffer: &mut Buffer) -> bool {
    find_universal_syllables(buffer);
    mark_syllables_unsafe_to_break(buffer);
    let Some(data) = plan_data(plan) else {
        return false;
    };
    if data.rphf != 0 {
        for_each_syllable(buffer, |buffer, start, end| {
            let limit = if buffer.info[start].shaper_category == cat::R {
                1
            } else {
                REPHA_LENGTH.min(end - start)
            };
            for info in &mut buffer.info[start..start + limit] {
                info.mask |= data.rphf;
            }
        });
    }
    if let Some(topography) = &data.topography {
        topography.setup_masks(buffer);
    }
    false
}

fn clear_substitution_flags(_: &PlanCore, _: &FontContext, buffer: &mut Buffer) -> bool {
    for info in &mut buffer.info[..buffer.len] {
        info.clear_substituted();
    }
    false
}

fn record_rphf(plan: &PlanCore, _: &FontContext, buffer: &mut Buffer) -> bool {
    let Some(mask) = plan_data(plan)
        .map(|data| data.rphf)
        .filter(|&mask| mask != 0)
    else {
        return false;
    };
    for_each_syllable(buffer, |buffer, start, end| {
        for info in &mut buffer.info[start..end] {
            if info.mask & mask == 0 {
                break;
            }
            if info.substituted() {
                info.shaper_category = cat::R;
                break;
            }
        }
    });
    false
}

fn record_pref(_: &PlanCore, _: &FontContext, buffer: &mut Buffer) -> bool {
    for_each_syllable(buffer, |buffer, start, end| {
        if let Some(info) = buffer.info[start..end]
            .iter_mut()
            .find(|info| info.substituted())
        {
            info.shaper_category = cat::VPRE;
        }
    });
    false
}

impl ComplexShaper for UniversalShaper {
    fn collect_features(&self, planner: &mut Planner<'_>) {
        let map = &mut planner.map;
        map.add_gsub_pause(Some(setup_syllables));
        map.enable_feature(Tag::new(b"locl"), flags::PER_SYLLABLE, 1);
        map.enable_feature(Tag::new(b"ccmp"), flags::PER_SYLLABLE, 1);
        map.enable_feature(Tag::new(b"nukt"), flags::PER_SYLLABLE, 1);
        map.enable_feature(
            Tag::new(b"akhn"),
            flags::MANUAL_ZWJ | flags::PER_SYLLABLE,
            1,
        );
        map.add_gsub_pause(Some(clear_substitution_flags));
        map.add_feature(
            Tag::new(b"rphf"),
            flags::MANUAL_ZWJ | flags::PER_SYLLABLE,
            1,
        );
        map.add_gsub_pause(Some(record_rphf));
        map.add_gsub_pause(Some(clear_substitution_flags));
        map.enable_feature(
            Tag::new(b"pref"),
            flags::MANUAL_ZWJ | flags::PER_SYLLABLE,
            1,
        );
        map.add_gsub_pause(Some(record_pref));
        for tag in BASIC_FEATURES {
            map.enable_feature(Tag::new(tag), flags::MANUAL_ZWJ | flags::PER_SYLLABLE, 1);
        }
        map.add_gsub_pause(Some(reorder));
        map.add_gsub_pause(Some(clear_syllables));
        for tag in TOPOGRAPHICAL_FEATURES {
            map.add_feature(Tag::new(tag), flags::NONE, 1);
        }
        map.add_gsub_pause(None);
        for tag in OTHER_FEATURES {
            map.enable_feature(Tag::new(tag), flags::MANUAL_ZWJ, 1);
        }
    }

    fn create_data(&self, plan: &PlanCore, _: &FontContext) -> Option<ShaperData> {
        let joining =
            has_arabic_joining(plan.properties.script).then(|| JoiningPlan::new(&plan.map));
        Some(Box::new(UniversalPlan {
            rphf: plan.map.one_mask(Tag::new(b"rphf")),
            topography: if joining.is_some() {
                None
            } else {
                Topography::new(&plan.map)
            },
            joining,
        }))
    }

    fn preprocess_text(&self, _: &PlanCore, _: &FontContext, buffer: &mut Buffer) {
        insert_dotted_circles_in_invalid_vowels(buffer);
    }

    fn normalization_mode(&self) -> NormalizationMode {
        NormalizationMode::ComposedDiacriticsNoShortCircuit
    }

    fn compose(&self, _: &NormalizeContext<'_>, first: u32, second: u32) -> Option<u32> {
        compose_unless_mark(first, second)
    }

    fn setup_masks(&self, plan: &PlanCore, _: &FontContext, buffer: &mut Buffer) {
        if let Some(joining) = plan_data(plan).and_then(|data| data.joining.as_ref()) {
            joining.setup_masks(buffer, plan.properties.script);
        }
        for info in &mut buffer.info[..buffer.len] {
            info.shaper_category = use_category(info.codepoint);
        }
    }

    fn zero_width_marks(&self) -> ZeroWidthMarks {
        ZeroWidthMarks::ByGdefEarly
    }

    fn fallback_position(&self) -> bool {
        false
    }
}
