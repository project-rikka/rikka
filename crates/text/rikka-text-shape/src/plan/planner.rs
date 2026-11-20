use std::sync::Arc;

use rikka_text_font::Tag;

use crate::common::feature::Feature;
use crate::common::segment_properties::SegmentProperties;
use crate::face::font_context::FontContext;
use crate::plan::feature_map::TableKind;
use crate::plan::layout_selection::TableSelection;
use crate::plan::map_builder::{MAX_VALUE, MapBuilder, feature_flags as flags};
use crate::plan::ot_tags::OpenTypeTags;
use crate::plan::shape_plan::PlanCore;
use crate::shapers::complex_shaper::{ComplexShaper, ZeroWidthMarks};
use crate::shapers::registry;

const COMMON_FEATURES: [(&[u8; 4], u32); 7] = [
    (b"abvm", flags::GLOBAL),
    (b"blwm", flags::GLOBAL),
    (b"ccmp", flags::GLOBAL),
    (b"locl", flags::GLOBAL),
    (b"mark", flags::GLOBAL_MANUAL_JOINERS),
    (b"mkmk", flags::GLOBAL_MANUAL_JOINERS),
    (b"rlig", flags::GLOBAL),
];

const HORIZONTAL_FEATURES: [(&[u8; 4], u32); 7] = [
    (b"calt", flags::GLOBAL),
    (b"clig", flags::GLOBAL),
    (b"curs", flags::GLOBAL),
    (b"dist", flags::GLOBAL),
    (b"kern", flags::GLOBAL_HAS_FALLBACK),
    (b"liga", flags::GLOBAL),
    (b"rclt", flags::GLOBAL),
];

#[derive(Debug)]
pub(crate) struct Planner<'a> {
    pub(crate) font: &'a FontContext,
    pub(crate) properties: SegmentProperties,
    pub(crate) map: MapBuilder<'a>,
    pub(crate) shaper: &'static dyn ComplexShaper,
}

impl<'a> Planner<'a> {
    fn new(font: &'a FontContext, properties: SegmentProperties) -> Self {
        let tags = OpenTypeTags::new(properties.script, properties.language.as_ref());
        let variations = font.feature_variations;
        let selections = [
            font.gsub()
                .and_then(|gsub| TableSelection::select(*gsub, &tags)),
            font.gpos()
                .and_then(|gpos| TableSelection::select(*gpos, &tags)),
        ];
        let map = MapBuilder::new(selections, variations);
        let shaper = registry::select(properties.script, map.chosen_script(TableKind::Gsub));
        Self {
            font,
            properties,
            map,
            shaper,
        }
    }

    fn collect_features(&mut self, user_features: &[Feature]) {
        let none = flags::NONE;
        self.map.enable_feature(Tag::new(b"rvrn"), none, 1);
        self.map.add_gsub_pause(None);
        if self.properties.direction.is_forward() {
            self.map.enable_feature(Tag::new(b"ltra"), none, 1);
            self.map.enable_feature(Tag::new(b"ltrm"), none, 1);
        } else {
            self.map.enable_feature(Tag::new(b"rtla"), none, 1);
            self.map.add_feature(Tag::new(b"rtlm"), none, 1);
        }
        self.map.add_feature(Tag::new(b"frac"), none, 1);
        self.map.add_feature(Tag::new(b"numr"), none, 1);
        self.map.add_feature(Tag::new(b"dnom"), none, 1);
        self.map
            .enable_feature(Tag::new(b"rand"), flags::RANDOM, MAX_VALUE);
        self.map
            .enable_feature(Tag::new(b"trak"), flags::HAS_FALLBACK, 1);
        self.map.enable_feature(Tag::new(b"Harf"), none, 1);
        self.map.enable_feature(Tag::new(b"HARF"), none, 1);
        let shaper = self.shaper;
        shaper.collect_features(self);
        self.map.enable_feature(Tag::new(b"Buzz"), none, 1);
        self.map.enable_feature(Tag::new(b"BUZZ"), none, 1);
        for (tag, feature_flags) in COMMON_FEATURES {
            self.map.add_feature(Tag::new(tag), feature_flags, 1);
        }
        for (tag, feature_flags) in HORIZONTAL_FEATURES {
            self.map.add_feature(Tag::new(tag), feature_flags, 1);
        }
        for feature in user_features {
            let feature_flags = if feature.is_global() {
                flags::GLOBAL
            } else {
                none
            };
            self.map
                .add_feature(feature.tag, feature_flags, feature.value);
        }
        shaper.override_features(self);
    }

    fn compile(self, user_features: &[Feature]) -> PlanCore {
        let shaper = self.shaper;
        let font = self.font;
        let mut map = self.map.compile();
        map.attach_digests(|table, index| {
            font.lookup(table, index).map(|accel| accel.filter.digest)
        });
        let frac_mask = map.one_mask(Tag::new(b"frac"));
        let numr_mask = map.one_mask(Tag::new(b"numr"));
        let dnom_mask = map.one_mask(Tag::new(b"dnom"));
        let rtlm_mask = map.one_mask(Tag::new(b"rtlm"));
        let kern_tag = Tag::new(b"kern");
        let kern_mask = map.mask(kern_tag).0;
        let has_gpos_kern = map.feature_index(TableKind::Gpos, kern_tag).is_some();
        let disable_gpos = shaper
            .gpos_tag()
            .is_some_and(|tag| Some(tag) != map.chosen_script(TableKind::Gpos));
        let apply_gpos = !disable_gpos && font.gpos().is_some();
        let script_fallback_position = shaper.fallback_position();
        let apply_kern =
            (!has_gpos_kern || !apply_gpos) && font.has_kern && script_fallback_position;
        let script_zero_marks = shaper.zero_width_marks() != ZeroWidthMarks::None;
        let zero_marks = script_zero_marks && (!apply_kern || !font.has_machine_kern);
        let adjust_mark_positioning_when_zeroing =
            !apply_gpos && (!apply_kern || !font.has_cross_kern);
        let mut plan = PlanCore {
            properties: self.properties,
            features: user_features.into(),
            shaper,
            shaper_data: None,
            has_gpos_mark: map.one_mask(Tag::new(b"mark")) != 0,
            map,
            frac_mask,
            numr_mask,
            dnom_mask,
            rtlm_mask,
            kern_mask,
            requested_kerning: kern_mask != 0,
            has_frac: frac_mask != 0 || (numr_mask != 0 && dnom_mask != 0),
            zero_marks,
            fallback_glyph_classes: !font.has_glyph_classes(),
            fallback_mark_positioning: adjust_mark_positioning_when_zeroing
                && script_fallback_position,
            adjust_mark_positioning_when_zeroing,
            apply_gpos,
            apply_kern,
        };
        plan.shaper_data = shaper.create_data(&plan, font);
        plan
    }
}

pub(crate) fn compile(
    font: &FontContext,
    properties: &SegmentProperties,
    features: &[Feature],
) -> Arc<PlanCore> {
    let mut planner = Planner::new(font, *properties);
    planner.collect_features(features);
    Arc::new(planner.compile(features))
}
