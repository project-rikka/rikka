use std::sync::atomic::{AtomicU8, Ordering};

use rikka_text_font::Tag;
use rikka_text_unicode::Script;

use crate::face::font_context::FontContext;
use crate::layout::would_apply::WouldApply;
use crate::plan::feature_map::{FeatureMap, TableKind};
use crate::plan::map_builder::feature_flags as flags;
use crate::plan::shape_plan::PlanCore;
use crate::shapers::indic::config::IndicConfig;

pub(crate) const FEATURES: [(&[u8; 4], u32); 17] = [
    (b"nukt", flags::GLOBAL_MANUAL_JOINERS | flags::PER_SYLLABLE),
    (b"akhn", flags::GLOBAL_MANUAL_JOINERS | flags::PER_SYLLABLE),
    (b"rphf", flags::MANUAL_JOINERS | flags::PER_SYLLABLE),
    (b"rkrf", flags::GLOBAL_MANUAL_JOINERS | flags::PER_SYLLABLE),
    (b"pref", flags::MANUAL_JOINERS | flags::PER_SYLLABLE),
    (b"blwf", flags::MANUAL_JOINERS | flags::PER_SYLLABLE),
    (b"abvf", flags::MANUAL_JOINERS | flags::PER_SYLLABLE),
    (b"half", flags::MANUAL_JOINERS | flags::PER_SYLLABLE),
    (b"pstf", flags::MANUAL_JOINERS | flags::PER_SYLLABLE),
    (b"vatu", flags::GLOBAL_MANUAL_JOINERS | flags::PER_SYLLABLE),
    (b"cjct", flags::GLOBAL_MANUAL_JOINERS | flags::PER_SYLLABLE),
    (b"init", flags::MANUAL_JOINERS | flags::PER_SYLLABLE),
    (b"pres", flags::GLOBAL_MANUAL_JOINERS | flags::PER_SYLLABLE),
    (b"abvs", flags::GLOBAL_MANUAL_JOINERS | flags::PER_SYLLABLE),
    (b"blws", flags::GLOBAL_MANUAL_JOINERS | flags::PER_SYLLABLE),
    (b"psts", flags::GLOBAL_MANUAL_JOINERS | flags::PER_SYLLABLE),
    (b"haln", flags::GLOBAL_MANUAL_JOINERS | flags::PER_SYLLABLE),
];

pub(crate) const BASIC_FEATURE_COUNT: usize = 11;
const UNKNOWN_POSITION: u8 = 0;

pub(crate) mod feature {
    pub(crate) const RPHF: usize = 2;
    pub(crate) const PREF: usize = 4;
    pub(crate) const BLWF: usize = 5;
    pub(crate) const ABVF: usize = 6;
    pub(crate) const HALF: usize = 7;
    pub(crate) const PSTF: usize = 8;
    pub(crate) const INIT: usize = 11;
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WouldSubstituteFeature {
    stage: Option<usize>,
}

impl WouldSubstituteFeature {
    fn new(map: &FeatureMap, tag: &[u8; 4]) -> Self {
        Self {
            stage: map.feature_stage(TableKind::Gsub, Tag::new(tag)),
        }
    }
}

#[derive(Debug)]
pub(crate) struct IndicPlan {
    pub(crate) config: IndicConfig,
    pub(crate) is_old_spec: bool,
    zero_context: bool,
    pub(crate) rphf: WouldSubstituteFeature,
    pub(crate) pref: WouldSubstituteFeature,
    pub(crate) blwf: WouldSubstituteFeature,
    pub(crate) pstf: WouldSubstituteFeature,
    pub(crate) vatu: WouldSubstituteFeature,
    pub(crate) masks: [u32; FEATURES.len()],
    consonant_positions: Box<[AtomicU8]>,
}

impl IndicPlan {
    pub(crate) fn new(plan: &PlanCore, font: &FontContext) -> Self {
        let script = plan.properties.script;
        let config = IndicConfig::for_script(script);
        let map = &plan.map;
        let is_old_spec = config.has_old_spec
            && map
                .chosen_script(TableKind::Gsub)
                .is_none_or(|tag| tag.to_bytes()[3] != b'2');
        let mut masks = [0; FEATURES.len()];
        for (mask, (tag, feature_flags)) in masks.iter_mut().zip(FEATURES) {
            if feature_flags & flags::GLOBAL == 0 {
                *mask = map.one_mask(Tag::new(tag));
            }
        }
        Self {
            config,
            is_old_spec,
            zero_context: !is_old_spec && script != Script::Malayalam,
            rphf: WouldSubstituteFeature::new(map, b"rphf"),
            pref: WouldSubstituteFeature::new(map, b"pref"),
            blwf: WouldSubstituteFeature::new(map, b"blwf"),
            pstf: WouldSubstituteFeature::new(map, b"pstf"),
            vatu: WouldSubstituteFeature::new(map, b"vatu"),
            masks,
            consonant_positions: (0..font.font.glyph_count())
                .map(|_| AtomicU8::new(UNKNOWN_POSITION))
                .collect(),
        }
    }

    pub(crate) fn consonant_position(&self, glyph: u32, compute: impl FnOnce() -> u8) -> u8 {
        let Some(cached) = self.consonant_positions.get(glyph as usize) else {
            return compute();
        };
        match cached.load(Ordering::Relaxed) {
            UNKNOWN_POSITION => {
                let position = compute();
                cached.store(position + 1, Ordering::Relaxed);
                position
            }
            known => known - 1,
        }
    }

    pub(crate) fn would_substitute(
        &self,
        plan: &PlanCore,
        font: &FontContext,
        probe: WouldSubstituteFeature,
        glyphs: &[u32],
    ) -> bool {
        probe.stage.is_some_and(|stage| {
            WouldApply::new(glyphs, self.zero_context)
                .any_lookup(font, plan.map.stage_lookups(TableKind::Gsub, stage))
        })
    }

    pub(crate) fn would_substitute_pair(
        &self,
        plan: &PlanCore,
        font: &FontContext,
        probe: WouldSubstituteFeature,
        first: u32,
        second: u32,
    ) -> bool {
        self.would_substitute(plan, font, probe, &[first, second])
            || self.would_substitute(plan, font, probe, &[second, first])
    }
}
