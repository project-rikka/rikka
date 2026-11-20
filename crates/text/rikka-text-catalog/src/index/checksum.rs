const SEED: u64 = 0x9E37_79B9_7F4A_7C15;
const MULTIPLIER: u64 = 0xFF51_AFD7_ED55_8CCD;

pub(crate) fn seal(mut bytes: Vec<u8>) -> Vec<u8> {
    let sum = checksum(&bytes);
    bytes.extend_from_slice(&sum.to_le_bytes());
    bytes
}

pub(crate) fn open(bytes: &[u8]) -> Option<&[u8]> {
    let (body, stored) = bytes.split_last_chunk::<8>()?;
    (checksum(body) == u64::from_le_bytes(*stored)).then_some(body)
}

fn checksum(bytes: &[u8]) -> u64 {
    let (words, tail) = bytes.as_chunks::<8>();
    let mut hash = SEED ^ bytes.len() as u64;
    for word in words {
        hash = (hash.rotate_left(29) ^ u64::from_le_bytes(*word)).wrapping_mul(MULTIPLIER);
    }
    for &byte in tail {
        hash = (hash.rotate_left(29) ^ u64::from(byte)).wrapping_mul(MULTIPLIER);
    }
    hash ^ (hash >> 32)
}
