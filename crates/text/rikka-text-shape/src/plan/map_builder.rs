use rikka_text_font::Tag;

use crate::buffer::glyph_info::glyph_flag;
use crate::face::digest::Digest;
use crate::plan::feature_map::{FeatureEntry, FeatureMap, LookupEntry, PauseFn, Stage, TableKind};
use crate::plan::layout_selection::TableSelection;

pub(crate) mod feature_flags {
    pub(crate) const NONE: u32 = 0x00;
    pub(crate) const GLOBAL: u32 = 0x01;
    pub(crate) const HAS_FALLBACK: u32 = 0x02;
    pub(crate) const MANUAL_ZWNJ: u32 = 0x04;
    pub(crate) const MANUAL_ZWJ: u32 = 0x08;
    pub(crate) const MANUAL_JOINERS: u32 = MANUAL_ZWNJ | MANUAL_ZWJ;
    pub(crate) const GLOBAL_MANUAL_JOINERS: u32 = GLOBAL | MANUAL_JOINERS;
    pub(crate) const GLOBAL_HAS_FALLBACK: u32 = GLOBAL | HAS_FALLBACK;
    pub(crate) const GLOBAL_SEARCH: u32 = 0x10;
    pub(crate) const RANDOM: u32 = 0x20;
    pub(crate) const PER_SYLLABLE: u32 = 0x40;
}

use feature_flags as flags;

pub(crate) const MAX_BITS: u32 = 8;
pub(crate) const MAX_VALUE: u32 = (1 << MAX_BITS) - 1;
const GLOBAL_BIT_SHIFT: u32 = u32::BITS - 1;
const GLOBAL_BIT_MASK: u32 = 1 << GLOBAL_BIT_SHIFT;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct FeatureInfo {
    tag: Tag,
    sequence: usize,
    max_value: u32,
    flags: u32,
    default_value: u32,
    stage: [usize; 2],
}

#[derive(Clone, Copy, Debug)]
struct StageInfo {
    index: usize,
    pause: Option<PauseFn>,
}

#[derive(Debug)]
pub(crate) struct MapBuilder<'a> {
    tables: [Option<TableSelection<'a>>; 2],
    variations: [Option<u32>; 2],
    current_stage: [usize; 2],
    features: Vec<FeatureInfo>,
    stages: [Vec<StageInfo>; 2],
}

impl<'a> MapBuilder<'a> {
    pub(crate) fn new(
        tables: [Option<TableSelection<'a>>; 2],
        variations: [Option<u32>; 2],
    ) -> Self {
        Self {
            tables,
            variations,
            current_stage: [0; 2],
            features: Vec::new(),
            stages: [Vec::new(), Vec::new()],
        }
    }

    pub(crate) fn chosen_script(&self, table: TableKind) -> Option<Tag> {
        self.tables[table.index()].map(|selection| selection.chosen_script)
    }

    pub(crate) fn has_feature(&self, tag: Tag) -> bool {
        self.tables
            .iter()
            .flatten()
            .any(|selection| selection.find_feature(tag).is_some())
    }

    pub(crate) fn add_feature(&mut self, tag: Tag, flags: u32, value: u32) {
        if tag.to_u32() == 0 {
            return;
        }
        self.features.push(FeatureInfo {
            tag,
            sequence: self.features.len(),
            max_value: value,
            flags,
            default_value: if flags & flags::GLOBAL != 0 { value } else { 0 },
            stage: self.current_stage,
        });
    }

    pub(crate) fn enable_feature(&mut self, tag: Tag, flags: u32, value: u32) {
        self.add_feature(tag, flags | flags::GLOBAL, value);
    }

    pub(crate) fn disable_feature(&mut self, tag: Tag) {
        self.add_feature(tag, flags::GLOBAL, 0);
    }

    pub(crate) fn add_gsub_pause(&mut self, pause: Option<PauseFn>) {
        self.add_pause(TableKind::Gsub, pause);
    }

    pub(crate) fn add_gpos_pause(&mut self, pause: Option<PauseFn>) {
        self.add_pause(TableKind::Gpos, pause);
    }

    fn add_pause(&mut self, table: TableKind, pause: Option<PauseFn>) {
        let index = table.index();
        self.stages[index].push(StageInfo {
            index: self.current_stage[index],
            pause,
        });
        self.current_stage[index] += 1;
    }

    pub(crate) fn compile(mut self) -> FeatureMap {
        let mut required_index = [None; 2];
        let mut required_tag = [None; 2];
        for table in TableKind::ALL {
            if let Some((index, tag)) = self.tables[table.index()]
                .as_ref()
                .and_then(TableSelection::required_feature)
            {
                required_index[table.index()] = Some(index);
                required_tag[table.index()] = Some(tag);
            }
        }
        let (features, required_stage, global_mask) = self.collect_features(required_tag);
        self.add_gsub_pause(None);
        self.add_gpos_pause(None);
        let (lookups, stages) = self.collect_lookups(&features, required_index, required_stage);
        FeatureMap {
            found_script: self
                .tables
                .map(|table| table.is_some_and(|selection| selection.found_script)),
            chosen_script: self
                .tables
                .map(|table| table.map(|selection| selection.chosen_script)),
            global_mask,
            features,
            lookups,
            stages,
        }
    }

    fn dedup_features(&mut self) {
        if self.features.is_empty() {
            return;
        }
        self.features.sort();
        let mut last_unique = 0;
        for index in 1..self.features.len() {
            let current = self.features[index];
            if current.tag != self.features[last_unique].tag {
                last_unique += 1;
                self.features[last_unique] = current;
                continue;
            }
            let target = &mut self.features[last_unique];
            if current.flags & flags::GLOBAL != 0 {
                target.flags |= flags::GLOBAL;
                target.max_value = current.max_value;
                target.default_value = current.default_value;
            } else {
                target.flags &= !flags::GLOBAL;
                target.max_value = target.max_value.max(current.max_value);
            }
            target.flags |= current.flags & flags::HAS_FALLBACK;
            target.stage[0] = target.stage[0].min(current.stage[0]);
            target.stage[1] = target.stage[1].min(current.stage[1]);
        }
        self.features.truncate(last_unique + 1);
    }

    fn collect_features(
        &mut self,
        required_tag: [Option<Tag>; 2],
    ) -> (Vec<FeatureEntry>, [usize; 2], u32) {
        let mut entries = Vec::new();
        let mut required_stage = [0; 2];
        let mut global_mask = GLOBAL_BIT_MASK;
        let mut next_bit = glyph_flag::DEFINED.count_ones() + 1;
        self.dedup_features();
        for info in &self.features {
            let uses_global_bit = info.flags & flags::GLOBAL != 0 && info.max_value == 1;
            let bits_needed = if uses_global_bit {
                0
            } else {
                MAX_BITS.min(u32::BITS - info.max_value.leading_zeros())
            };
            if info.max_value == 0 || next_bit + bits_needed >= GLOBAL_BIT_SHIFT {
                continue;
            }
            let mut found = false;
            let mut index = [None; 2];
            for table in TableKind::ALL {
                let slot = table.index();
                if required_tag[slot] == Some(info.tag) {
                    required_stage[slot] = info.stage[slot];
                }
                if let Some(feature) = self.tables[slot]
                    .as_ref()
                    .and_then(|selection| selection.find_feature(info.tag))
                {
                    index[slot] = Some(feature);
                    found = true;
                }
            }
            if !found && info.flags & flags::GLOBAL_SEARCH != 0 {
                for table in TableKind::ALL {
                    let slot = table.index();
                    if let Some(feature) = self.tables[slot]
                        .as_ref()
                        .and_then(|selection| selection.find_any_feature(info.tag))
                    {
                        index[slot] = Some(feature);
                        found = true;
                    }
                }
            }
            if !found && info.flags & flags::HAS_FALLBACK == 0 {
                continue;
            }
            let (shift, mask) = if uses_global_bit {
                (GLOBAL_BIT_SHIFT, GLOBAL_BIT_MASK)
            } else {
                let shift = next_bit;
                let mask = (1u32 << (next_bit + bits_needed)) - (1u32 << next_bit);
                next_bit += bits_needed;
                global_mask |= (info.default_value << shift) & mask;
                (shift, mask)
            };
            entries.push(FeatureEntry {
                tag: info.tag,
                index,
                stage: info.stage,
                shift,
                mask,
                one_mask: (1 << shift) & mask,
                auto_zwnj: info.flags & flags::MANUAL_ZWNJ == 0,
                auto_zwj: info.flags & flags::MANUAL_ZWJ == 0,
                random: info.flags & flags::RANDOM != 0,
                per_syllable: info.flags & flags::PER_SYLLABLE != 0,
                needs_fallback: !found,
            });
        }
        (entries, required_stage, global_mask)
    }

    fn collect_lookups(
        &self,
        features: &[FeatureEntry],
        required_index: [Option<u16>; 2],
        required_stage: [usize; 2],
    ) -> ([Vec<LookupEntry>; 2], [Vec<Stage>; 2]) {
        let mut lookups = [Vec::new(), Vec::new()];
        let mut stages = [Vec::new(), Vec::new()];
        for table in TableKind::ALL {
            let slot = table.index();
            let mut stage_index = 0;
            let mut last_lookup = 0;
            for stage in 0..self.current_stage[slot] {
                if let Some(feature) = required_index[slot]
                    && required_stage[slot] == stage
                {
                    self.add_lookups(
                        &mut lookups[slot],
                        slot,
                        feature,
                        GLOBAL_BIT_MASK,
                        [true, true, false, false],
                    );
                }
                for feature in features {
                    if let Some(index) = feature.index[slot]
                        && feature.stage[slot] == stage
                    {
                        self.add_lookups(
                            &mut lookups[slot],
                            slot,
                            index,
                            feature.mask,
                            [
                                feature.auto_zwnj,
                                feature.auto_zwj,
                                feature.random,
                                feature.per_syllable,
                            ],
                        );
                    }
                }
                let list = &mut lookups[slot];
                if last_lookup + 1 < list.len() {
                    list[last_lookup..].sort_by_key(|lookup: &LookupEntry| {
                        (
                            lookup.index,
                            lookup.auto_zwnj,
                            lookup.auto_zwj,
                            lookup.random,
                            lookup.mask,
                            lookup.per_syllable,
                        )
                    });
                    let mut last_unique = last_lookup;
                    for index in last_unique + 1..list.len() {
                        if list[index].index != list[last_unique].index {
                            last_unique += 1;
                            list[last_unique] = list[index];
                        } else {
                            let merged = list[index];
                            let target = &mut list[last_unique];
                            target.mask |= merged.mask;
                            target.auto_zwnj &= merged.auto_zwnj;
                            target.auto_zwj &= merged.auto_zwj;
                        }
                    }
                    list.truncate(last_unique + 1);
                }
                last_lookup = list.len();
                if let Some(info) = self.stages[slot].get(stage_index)
                    && info.index == stage
                {
                    stages[slot].push(Stage {
                        last_lookup,
                        pause: info.pause,
                    });
                    stage_index += 1;
                }
            }
        }
        (lookups, stages)
    }

    fn add_lookups(
        &self,
        lookups: &mut Vec<LookupEntry>,
        slot: usize,
        feature_index: u16,
        mask: u32,
        [auto_zwnj, auto_zwj, random, per_syllable]: [bool; 4],
    ) {
        let Some(selection) = self.tables[slot] else {
            return;
        };
        let table = selection.table;
        let Some(feature) = table.feature(feature_index, self.variations[slot]) else {
            return;
        };
        let count = table.lookup_count();
        for index in feature.lookup_indices() {
            if usize::from(index) < count {
                lookups.push(LookupEntry {
                    index,
                    digest: Digest::default(),
                    mask,
                    auto_zwnj,
                    auto_zwj,
                    random,
                    per_syllable,
                });
            }
        }
    }
}
