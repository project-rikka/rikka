use crate::layout::feature_list::{Feature, FeatureList};
use crate::parse::bytes::Bytes;
use crate::parse::f2dot14::F2Dot14;

const RECORD_SIZE: usize = 8;

#[derive(Clone, Copy, Debug)]
pub struct FeatureVariations<'a> {
    data: &'a [u8],
    records: &'a [u8],
}

#[derive(Clone, Copy, Debug)]
pub struct FeatureSubstitution<'a> {
    data: &'a [u8],
    records: &'a [u8],
}

impl<'a> FeatureVariations<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        if data.u16_at(0)? != 1 {
            return None;
        }
        let count = data.u32_at(4)? as usize;
        let records = data.range(8, count.checked_mul(RECORD_SIZE)?)?;
        Some(Self { data, records })
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.records.len() / RECORD_SIZE
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn find_index(&self, coords: &[F2Dot14]) -> Option<u32> {
        (0..self.len())
            .find(|&index| self.matches(index, coords))
            .map(|index| index as u32)
    }

    pub fn matches(&self, index: usize, coords: &[F2Dot14]) -> bool {
        let Some(offset) = self.records.u32_at(index * RECORD_SIZE) else {
            return false;
        };
        if offset == 0 {
            return true;
        }
        let Some(set) = self.data.tail(offset as usize) else {
            return false;
        };
        let Some(count) = set.u16_at(0) else {
            return false;
        };
        (0..usize::from(count)).all(|condition| {
            set.offset32(0, 2 + condition * 4)
                .is_some_and(|condition| condition_matches(condition, coords))
        })
    }

    pub fn substitution(&self, index: u32) -> Option<FeatureSubstitution<'a>> {
        let at = (index as usize).checked_mul(RECORD_SIZE)? + 4;
        if at + 4 > self.records.len() {
            return None;
        }
        let offset = self.records.u32_at(at)?;
        let data = self.data.tail(offset as usize)?;
        let count = usize::from(data.u16_at(4)?);
        let records = data.range(6, count * 6)?;
        Some(FeatureSubstitution { data, records })
    }
}

fn condition_matches(condition: &[u8], coords: &[F2Dot14]) -> bool {
    let (Some(1), Some(axis), Some(min), Some(max)) = (
        condition.u16_at(0),
        condition.u16_at(2),
        condition.i16_at(4),
        condition.i16_at(6),
    ) else {
        return false;
    };
    let coord = coords.get(usize::from(axis)).map_or(0, |c| c.0);
    (min..=max).contains(&coord)
}

impl<'a> FeatureSubstitution<'a> {
    #[inline]
    pub fn len(&self) -> usize {
        self.records.len() / 6
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn find(&self, feature_index: u16, features: &FeatureList<'a>) -> Option<Feature<'a>> {
        let mut low = 0;
        let mut high = self.len();
        while low < high {
            let middle = low + (high - low) / 2;
            let index = self.records.u16_at(middle * 6)?;
            match index.cmp(&feature_index) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => {
                    let offset = self.records.u32_at(middle * 6 + 2)?;
                    let data = self.data.tail(offset as usize)?;
                    return Feature::new(features.tag(feature_index)?, data);
                }
            }
        }
        None
    }
}
