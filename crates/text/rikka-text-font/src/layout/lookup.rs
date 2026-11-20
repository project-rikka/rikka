use std::marker::PhantomData;

use crate::layout::gpos::subtable::PosSubtable;
use crate::layout::gsub::subtable::SubstSubtable;
use crate::layout::lookup_flag::LookupFlag;
use crate::layout::subtable_location::SubtableLocation;
use crate::parse::array::Array;
use crate::parse::bytes::Bytes;

pub trait LookupSubtable<'a>: Sized {
    const EXTENSION_KIND: u16;

    fn parse(kind: u16, data: &'a [u8]) -> Option<Self>;

    #[inline(always)]
    fn from_location(table: &'a [u8], location: SubtableLocation) -> Option<Self> {
        Self::parse(location.kind(), table.tail(location.offset())?)
    }
}

impl<'a> LookupSubtable<'a> for SubstSubtable<'a> {
    const EXTENSION_KIND: u16 = SubstSubtable::EXTENSION;

    #[inline(always)]
    fn parse(kind: u16, data: &'a [u8]) -> Option<Self> {
        Self::new(kind, data)
    }
}

impl<'a> LookupSubtable<'a> for PosSubtable<'a> {
    const EXTENSION_KIND: u16 = PosSubtable::EXTENSION;

    #[inline(always)]
    fn parse(kind: u16, data: &'a [u8]) -> Option<Self> {
        Self::new(kind, data)
    }
}

#[derive(Debug)]
pub struct Lookup<'a, S> {
    table: &'a [u8],
    offset: usize,
    kind: u16,
    extension: bool,
    flag: LookupFlag,
    offsets: Array<'a, u16>,
    mark_filtering_set: Option<u16>,
    subtable: PhantomData<S>,
}

impl<S> Clone for Lookup<'_, S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S> Copy for Lookup<'_, S> {}

pub type SubstLookup<'a> = Lookup<'a, SubstSubtable<'a>>;
pub type PosLookup<'a> = Lookup<'a, PosSubtable<'a>>;

impl<'a, S: LookupSubtable<'a>> Lookup<'a, S> {
    pub(crate) fn new(table: &'a [u8], offset: usize) -> Option<Self> {
        let data = table.tail(offset)?;
        let raw_kind = data.u16_at(0)?;
        let flag = LookupFlag(data.u16_at(2)?);
        let count = usize::from(data.u16_at(4)?);
        let offsets = data.array::<u16>(6, count)?;
        let mark_filtering_set = if flag.use_mark_filtering_set() {
            Some(data.u16_at(6 + count * 2)?)
        } else {
            None
        };
        let extension = raw_kind == S::EXTENSION_KIND;
        let mut lookup = Self {
            table,
            offset,
            kind: raw_kind,
            extension,
            flag,
            offsets,
            mark_filtering_set,
            subtable: PhantomData,
        };
        if extension {
            lookup.kind = lookup
                .offsets
                .get(0)
                .and_then(|first| data.u16_at(usize::from(first) + 2))
                .unwrap_or(0);
        }
        Some(lookup)
    }

    #[inline]
    pub fn kind(&self) -> u16 {
        self.kind
    }

    #[inline]
    pub fn flag(&self) -> LookupFlag {
        self.flag
    }

    #[inline]
    pub fn mark_filtering_set(&self) -> Option<u16> {
        self.mark_filtering_set
    }

    #[inline]
    pub fn subtable_count(&self) -> usize {
        self.offsets.len()
    }

    pub fn subtable_location(&self, index: usize) -> Option<SubtableLocation> {
        let mut offset = self
            .offset
            .checked_add(usize::from(self.offsets.get(index)?))?;
        if self.extension {
            let data = self.table.tail(offset)?;
            if data.u16_at(0)? != 1
                || data.u16_at(2)? != self.kind
                || self.kind == S::EXTENSION_KIND
            {
                return None;
            }
            let target = data.u32_at(4)?;
            if target == 0 {
                return None;
            }
            offset = offset.checked_add(target as usize)?;
        }
        if offset > self.table.len() {
            return None;
        }
        SubtableLocation::new(offset, self.kind)
    }

    #[inline]
    pub fn subtable(&self, index: usize) -> Option<S> {
        S::from_location(self.table, self.subtable_location(index)?)
    }

    pub fn subtables(&self) -> impl Iterator<Item = S> + use<'a, S> {
        let lookup = *self;
        (0..self.subtable_count()).filter_map(move |index| lookup.subtable(index))
    }
}
