use crate::inflate::bit_reader::BitReader;

const PRIMARY_BITS: u32 = 10;
const PRIMARY_MASK: u64 = (1 << PRIMARY_BITS) - 1;
const PRIMARY_SIZE: usize = 1 << PRIMARY_BITS;
const LINK: u32 = 1 << 31;
const MAX_LENGTH: usize = 15;

#[derive(Debug, Default)]
pub(crate) struct DecodingTable {
    table: Vec<u32>,
}

impl DecodingTable {
    pub(crate) fn build(&mut self, lengths: &[u8]) -> bool {
        let mut counts = [0u32; MAX_LENGTH + 1];
        for &length in lengths {
            let Some(count) = counts.get_mut(usize::from(length)) else {
                return false;
            };
            *count += 1;
        }
        counts[0] = 0;
        let mut left = 1i64;
        for &count in &counts[1..] {
            left = (left << 1) - i64::from(count);
            if left < 0 {
                return false;
            }
        }
        let mut next = [0u32; MAX_LENGTH + 1];
        let mut code = 0;
        for length in 1..=MAX_LENGTH {
            code = (code + counts[length - 1]) << 1;
            next[length] = code;
        }
        self.table.clear();
        self.table.resize(PRIMARY_SIZE, 0);
        let mut longest = [0u8; PRIMARY_SIZE];
        let mut probe = next;
        for &length in lengths {
            if usize::from(length) > PRIMARY_BITS as usize {
                let code = probe[usize::from(length)];
                probe[usize::from(length)] += 1;
                let prefix = reverse(code, u32::from(length)) as usize & (PRIMARY_SIZE - 1);
                longest[prefix] = longest[prefix].max(length);
            }
        }
        for (prefix, &length) in longest.iter().enumerate() {
            if length == 0 {
                continue;
            }
            let sub_bits = u32::from(length) - PRIMARY_BITS;
            let offset = self.table.len() as u32;
            self.table.resize(self.table.len() + (1 << sub_bits), 0);
            self.table[prefix] = LINK | (sub_bits << 16) | offset;
        }
        for (symbol, &length) in lengths.iter().enumerate() {
            if length == 0 {
                continue;
            }
            let length = u32::from(length);
            let code = next[length as usize];
            next[length as usize] += 1;
            let reversed = reverse(code, length) as usize;
            if length <= PRIMARY_BITS {
                let entry = (length << 16) | symbol as u32;
                for index in (reversed..PRIMARY_SIZE).step_by(1 << length) {
                    self.table[index] = entry;
                }
            } else {
                let link = self.table[reversed & (PRIMARY_SIZE - 1)];
                let sub_bits = (link >> 16) & 0x1F;
                let offset = (link & 0xFFFF) as usize;
                let remaining = length - PRIMARY_BITS;
                let entry = (remaining << 16) | symbol as u32;
                for index in ((reversed >> PRIMARY_BITS)..(1 << sub_bits)).step_by(1 << remaining) {
                    self.table[offset + index] = entry;
                }
            }
        }
        true
    }

    #[inline]
    pub(crate) fn decode(&self, bits: &mut BitReader<'_>) -> Option<u16> {
        let window = bits.peek();
        let entry = self.table[(window & PRIMARY_MASK) as usize];
        if entry & LINK == 0 {
            let length = (entry >> 16) & 0x1F;
            if length == 0 {
                return None;
            }
            bits.consume(length);
            return Some(entry as u16);
        }
        let sub_bits = (entry >> 16) & 0x1F;
        let offset = (entry & 0xFFFF) as usize;
        let index = ((window >> PRIMARY_BITS) & ((1 << sub_bits) - 1)) as usize;
        let entry = *self.table.get(offset + index)?;
        let length = (entry >> 16) & 0x1F;
        if length == 0 {
            return None;
        }
        bits.consume(PRIMARY_BITS + length);
        Some(entry as u16)
    }
}

#[inline]
fn reverse(code: u32, length: u32) -> u32 {
    code.reverse_bits() >> (32 - length)
}
