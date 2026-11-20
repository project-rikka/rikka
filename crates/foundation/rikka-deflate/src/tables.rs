pub(crate) const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
pub(crate) const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
pub(crate) const DISTANCE_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
pub(crate) const DISTANCE_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
pub(crate) const CODE_LENGTH_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];
pub(crate) const END_OF_BLOCK: u16 = 256;
pub(crate) const FIRST_LENGTH: u16 = 257;
pub(crate) const LITERAL_CODES: usize = 286;
pub(crate) const LITERAL_SYMBOLS: usize = 288;
pub(crate) const DISTANCE_CODES: usize = 30;
pub(crate) const CODE_LENGTH_CODES: usize = 19;
pub(crate) const MAX_CODE_BITS: usize = 15;
pub(crate) const MIN_MATCH: usize = 3;
pub(crate) const MAX_MATCH: usize = 258;
pub(crate) const WINDOW: usize = 32768;

pub(crate) fn fixed_literal_lengths() -> [u8; LITERAL_SYMBOLS] {
    let mut lengths = [0u8; LITERAL_SYMBOLS];
    lengths[..144].fill(8);
    lengths[144..256].fill(9);
    lengths[256..280].fill(7);
    lengths[280..].fill(8);
    lengths
}

pub(crate) const FIXED_DISTANCE_LENGTHS: [u8; DISTANCE_CODES] = [5; DISTANCE_CODES];
