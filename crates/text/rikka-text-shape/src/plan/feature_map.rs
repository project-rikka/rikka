use std::ops::Range;

use rikka_text_font::Tag;

use crate::buffer::glyph_buffer::Buffer;
use crate::face::digest::Digest;
use crate::face::font_context::FontContext;
use crate::plan::shape_plan::PlanCore;

pub(crate) type PauseFn = fn(&PlanCore, &FontContext, &mut Buffer) -> bool;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TableKind {
    Gsub,
    Gpos,
}

impl TableKind {
    pub(crate) const ALL: [Self; 2] = [Self::Gsub, Self::Gpos];

    #[inline]
    pub(crate) fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FeatureEntry {
    pub(crate) tag: Tag,
    pub(crate) index: [Option<u16>; 2],
    pub(crate) stage: [usize; 2],
    pub(crate) shift: u32,
    pub(crate) mask: u32,
    pub(crate) one_mask: u32,
    pub(crate) auto_zwnj: bool,
    pub(crate) auto_zwj: bool,
    pub(crate) random: bool,
    pub(crate) per_syllable: bool,
    pub(crate) needs_fallback: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LookupEntry {
    pub(crate) index: u16,
    pub(crate) digest: Digest,
    pub(crate) mask: u32,
    pub(crate) auto_zwnj: bool,
    pub(crate) auto_zwj: bool,
    pub(crate) random: bool,
    pub(crate) per_syllable: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Stage {
    pub(crate) last_lookup: usize,
    pub(crate) pause: Option<PauseFn>,
}

#[derive(Debug)]
pub(crate) struct FeatureMap {
    pub(crate) found_script: [bool; 2],
    pub(crate) chosen_script: [Option<Tag>; 2],
    pub(crate) global_mask: u32,
    pub(crate) features: Vec<FeatureEntry>,
    pub(crate) lookups: [Vec<LookupEntry>; 2],
    pub(crate) stages: [Vec<Stage>; 2],
}

impl FeatureMap {
    fn entry(&self, tag: Tag) -> Option<&FeatureEntry> {
        self.features
            .binary_search_by_key(&tag, |feature| feature.tag)
            .ok()
            .map(|index| &self.features[index])
    }

    #[inline]
    pub(crate) fn mask(&self, tag: Tag) -> (u32, u32) {
        self.entry(tag)
            .map_or((0, 0), |feature| (feature.mask, feature.shift))
    }

    #[inline]
    pub(crate) fn one_mask(&self, tag: Tag) -> u32 {
        self.entry(tag).map_or(0, |feature| feature.one_mask)
    }

    pub(crate) fn needs_fallback(&self, tag: Tag) -> bool {
        self.entry(tag)
            .is_some_and(|feature| feature.needs_fallback)
    }

    pub(crate) fn feature_index(&self, table: TableKind, tag: Tag) -> Option<u16> {
        self.entry(tag)?.index[table.index()]
    }

    pub(crate) fn feature_stage(&self, table: TableKind, tag: Tag) -> Option<usize> {
        Some(self.entry(tag)?.stage[table.index()])
    }

    #[inline]
    pub(crate) fn found_script(&self, table: TableKind) -> bool {
        self.found_script[table.index()]
    }

    #[inline]
    pub(crate) fn chosen_script(&self, table: TableKind) -> Option<Tag> {
        self.chosen_script[table.index()]
    }

    #[inline]
    pub(crate) fn stages(&self, table: TableKind) -> &[Stage] {
        &self.stages[table.index()]
    }

    pub(crate) fn stage_lookups(&self, table: TableKind, stage: usize) -> &[LookupEntry] {
        &self.lookups[table.index()][self.stage_range(table, stage)]
    }

    fn stage_range(&self, table: TableKind, stage: usize) -> Range<usize> {
        let stages = &self.stages[table.index()];
        let lookups = &self.lookups[table.index()];
        let start = stage
            .checked_sub(1)
            .map_or(0, |previous| stages[previous].last_lookup);
        let end = stages
            .get(stage)
            .map_or(lookups.len(), |current| current.last_lookup);
        start..end
    }
}

impl FeatureMap {
    pub(crate) fn attach_digests(&mut self, digest_of: impl Fn(TableKind, u16) -> Option<Digest>) {
        for table in TableKind::ALL {
            for entry in &mut self.lookups[table.index()] {
                entry.digest = digest_of(table, entry.index).unwrap_or_default();
            }
        }
    }
}

impl FeatureEntry {
    fn is_equivalent(&self, other: &Self) -> bool {
        self.tag == other.tag
            && self.shift == other.shift
            && self.mask == other.mask
            && self.one_mask == other.one_mask
            && self.auto_zwnj == other.auto_zwnj
            && self.auto_zwj == other.auto_zwj
            && self.random == other.random
            && self.per_syllable == other.per_syllable
            && self.needs_fallback == other.needs_fallback
    }
}

impl LookupEntry {
    fn is_equivalent(&self, other: &Self) -> bool {
        self.index == other.index
            && self.mask == other.mask
            && self.auto_zwnj == other.auto_zwnj
            && self.auto_zwj == other.auto_zwj
            && self.random == other.random
            && self.per_syllable == other.per_syllable
    }
}

impl Stage {
    fn is_equivalent(&self, other: &Self) -> bool {
        self.last_lookup == other.last_lookup
            && match (self.pause, other.pause) {
                (None, None) => true,
                (Some(left), Some(right)) => std::ptr::fn_addr_eq(left, right),
                _ => false,
            }
    }
}

fn all_equivalent<T>(left: &[T], right: &[T], equivalent: impl Fn(&T, &T) -> bool) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| equivalent(left, right))
}

impl FeatureMap {
    pub(crate) fn is_equivalent(&self, other: &Self) -> bool {
        self.found_script == other.found_script
            && self.global_mask == other.global_mask
            && all_equivalent(&self.features, &other.features, FeatureEntry::is_equivalent)
            && TableKind::ALL.into_iter().all(|table| {
                let index = table.index();
                all_equivalent(
                    &self.lookups[index],
                    &other.lookups[index],
                    LookupEntry::is_equivalent,
                ) && all_equivalent(
                    &self.stages[index],
                    &other.stages[index],
                    Stage::is_equivalent,
                )
            })
    }
}
