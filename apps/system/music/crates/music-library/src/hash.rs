const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const SEPARATOR: u8 = 0x1f;

pub(crate) fn stable_hash<'a>(parts: impl IntoIterator<Item = &'a str>) -> i64 {
    let mut hash = FNV_OFFSET;
    for part in parts {
        for byte in part.bytes().chain(std::iter::once(SEPARATOR)) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    i64::try_from(hash >> 1).unwrap_or(i64::MAX)
}
