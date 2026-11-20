use std::fmt;

use rand::CryptoRng;

const UUID_BYTES: usize = 16;
const VERSION_BYTE: usize = 6;
const VARIANT_BYTE: usize = 8;
const GROUPS: [std::ops::Range<usize>; 5] = [0..4, 4..6, 6..8, 8..10, 10..16];

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DeviceId(String);

impl DeviceId {
    pub fn generate<R: CryptoRng + ?Sized>(rng: &mut R) -> DeviceId {
        let mut bytes = [0u8; UUID_BYTES];
        rng.fill_bytes(&mut bytes);
        bytes[VERSION_BYTE] = (bytes[VERSION_BYTE] & 0x0f) | 0x40;
        bytes[VARIANT_BYTE] = (bytes[VARIANT_BYTE] & 0x3f) | 0x80;
        let groups: Vec<String> = GROUPS
            .iter()
            .map(|group| rikka_types::hex::encode(&bytes[group.clone()]))
            .collect();
        DeviceId(groups.join("-"))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
