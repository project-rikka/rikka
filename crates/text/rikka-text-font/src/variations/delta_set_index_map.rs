use crate::parse::bytes::Bytes;
use crate::variations::item_variation_store::DeltaSetIndex;

const INNER_BIT_COUNT_MASK: u8 = 0x0F;
const ENTRY_SIZE_MASK: u8 = 0x30;

#[derive(Clone, Copy, Debug)]
pub struct DeltaSetIndexMap<'a> {
    entries: &'a [u8],
    count: u32,
    entry_size: usize,
    inner_bits: u32,
}

impl<'a> DeltaSetIndexMap<'a> {
    pub fn new(data: &'a [u8]) -> Option<Self> {
        let format = data.u8_at(0)?;
        let entry_format = data.u8_at(1)?;
        let (count, header) = match format {
            0 => (u32::from(data.u16_at(2)?), 4),
            1 => (data.u32_at(2)?, 6),
            _ => return None,
        };
        let entry_size = usize::from(((entry_format & ENTRY_SIZE_MASK) >> 4) + 1);
        let inner_bits = u32::from(entry_format & INNER_BIT_COUNT_MASK) + 1;
        let entries = data.range(header, (count as usize).checked_mul(entry_size)?)?;
        Some(Self {
            entries,
            count,
            entry_size,
            inner_bits,
        })
    }

    #[inline]
    pub fn get(&self, index: u32) -> Option<DeltaSetIndex> {
        let index = index.min(self.count.checked_sub(1)?) as usize;
        let start = index * self.entry_size;
        let bytes = self.entries.range(start, self.entry_size)?;
        let entry = bytes
            .iter()
            .fold(0u32, |entry, &byte| (entry << 8) | u32::from(byte));
        Some(DeltaSetIndex {
            outer: (entry >> self.inner_bits) as u16,
            inner: (entry & ((1 << self.inner_bits) - 1)) as u16,
        })
    }
}
