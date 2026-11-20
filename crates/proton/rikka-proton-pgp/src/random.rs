use rand::RngCore as _;

#[must_use]
pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    fill_random(&mut bytes);
    bytes
}

pub fn fill_random(bytes: &mut [u8]) {
    rand::rng().fill_bytes(bytes);
}

#[must_use]
pub fn random_hex(length: usize) -> String {
    let mut bytes = vec![0u8; length];
    fill_random(&mut bytes);
    rikka_types::hex::encode(bytes)
}

#[must_use]
pub fn random_uuid() -> String {
    let mut bytes = random_bytes::<16>();
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = rikka_types::hex::encode(bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}
