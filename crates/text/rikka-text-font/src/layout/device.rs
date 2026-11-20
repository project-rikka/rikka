use crate::parse::bytes::Bytes;
use crate::parse::f2dot14::F2Dot14;
use crate::variations::item_variation_store::{DeltaSetIndex, ItemVariationStore};

const VARIATION_INDEX: u16 = 0x8000;

#[derive(Clone, Copy, Debug)]
pub enum Device<'a> {
    Hinting(HintingDevice<'a>),
    Variation(DeltaSetIndex),
}

#[derive(Clone, Copy, Debug)]
pub struct HintingDevice<'a> {
    start_size: u16,
    end_size: u16,
    bits: u16,
    deltas: &'a [u8],
}

impl<'a> Device<'a> {
    #[inline]
    pub fn new(data: &'a [u8]) -> Option<Self> {
        let first = data.u16_at(0)?;
        let second = data.u16_at(2)?;
        let format = data.u16_at(4)?;
        let bits = match format {
            1 => 2,
            2 => 4,
            3 => 8,
            VARIATION_INDEX => {
                return Some(Self::Variation(DeltaSetIndex {
                    outer: first,
                    inner: second,
                }));
            }
            _ => return None,
        };
        Some(Self::Hinting(HintingDevice {
            start_size: first,
            end_size: second,
            bits,
            deltas: data.tail(6)?,
        }))
    }

    #[inline]
    pub fn hinting_delta(&self, ppem: u16) -> i32 {
        match self {
            Self::Hinting(device) => device.delta(ppem),
            Self::Variation(_) => 0,
        }
    }

    #[inline]
    pub fn variation_delta(
        &self,
        coords: &[F2Dot14],
        store: Option<&ItemVariationStore<'_>>,
    ) -> f32 {
        match (self, store) {
            (Self::Variation(index), Some(store)) => store.delta(*index, coords),
            _ => 0.0,
        }
    }
}

impl HintingDevice<'_> {
    #[inline]
    pub fn delta(&self, ppem: u16) -> i32 {
        if ppem < self.start_size || ppem > self.end_size {
            return 0;
        }
        let index = usize::from(ppem - self.start_size);
        let bits = usize::from(self.bits);
        let per_word = 16 / bits;
        let Some(word) = self.deltas.u16_at(index / per_word * 2) else {
            return 0;
        };
        let shift = 16 - bits * (index % per_word + 1);
        let raw = i32::from((word >> shift) & ((1u16 << bits) - 1));
        let sign = 1i32 << (bits - 1);
        if raw >= sign { raw - (sign << 1) } else { raw }
    }
}
