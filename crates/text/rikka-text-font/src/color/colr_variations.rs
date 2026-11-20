use crate::parse::f2dot14::F2Dot14;
use crate::variations::delta_set_index_map::DeltaSetIndexMap;
use crate::variations::item_variation_store::{DeltaSetIndex, ItemVariationStore};

const NO_VARIATION: u32 = 0xFFFF_FFFF;
const F2DOT14_UNIT: f32 = 16384.0;
const FIXED_UNIT: f32 = 65536.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ColrVariations<'c> {
    store: Option<ItemVariationStore<'c>>,
    map: Option<DeltaSetIndexMap<'c>>,
    coords: &'c [F2Dot14],
}

impl<'c> ColrVariations<'c> {
    pub(crate) fn new(
        store: Option<ItemVariationStore<'c>>,
        map: Option<DeltaSetIndexMap<'c>>,
        coords: &'c [F2Dot14],
    ) -> Self {
        let active = coords.iter().any(|coord| coord.0 != 0);
        Self {
            store: store.filter(|_| active),
            map,
            coords,
        }
    }

    pub(crate) fn delta(&self, base: u32, field: u32) -> f32 {
        let Some(store) = self.store else {
            return 0.0;
        };
        if base == NO_VARIATION {
            return 0.0;
        }
        let Some(index) = base.checked_add(field) else {
            return 0.0;
        };
        let index = match self.map {
            Some(map) => match map.get(index) {
                Some(index) => index,
                None => return 0.0,
            },
            None => DeltaSetIndex::from_packed(index),
        };
        store.delta(index, self.coords)
    }

    #[inline]
    pub(crate) fn fword(&self, value: i16, base: u32, field: u32) -> f32 {
        f32::from(value) + self.delta(base, field)
    }

    #[inline]
    pub(crate) fn ufword(&self, value: u16, base: u32, field: u32) -> f32 {
        f32::from(value) + self.delta(base, field)
    }

    #[inline]
    pub(crate) fn f2dot14(&self, value: i16, base: u32, field: u32) -> f32 {
        (f32::from(value) + self.delta(base, field)) / F2DOT14_UNIT
    }

    #[inline]
    pub(crate) fn fixed(&self, value: i32, base: u32, field: u32) -> f32 {
        (value as f32 + self.delta(base, field)) / FIXED_UNIT
    }
}
