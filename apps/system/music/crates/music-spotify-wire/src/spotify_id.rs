use std::fmt;

use crate::error::{Error, Result};

const BASE62: &[u8; 62] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
const BASE62_LEN: usize = 22;
const RADIX: u128 = 62;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpotifyId(u128);

impl SpotifyId {
    pub const GID_LEN: usize = 16;

    pub fn from_base62(text: &str) -> Result<SpotifyId> {
        if text.len() != BASE62_LEN {
            return Err(Error::Id(text.to_owned()));
        }
        text.bytes()
            .try_fold(0u128, |value, character| {
                let digit = match character {
                    b'0'..=b'9' => character - b'0',
                    b'a'..=b'z' => character - b'a' + 10,
                    b'A'..=b'Z' => character - b'A' + 36,
                    _ => return None,
                };
                value.checked_mul(RADIX)?.checked_add(u128::from(digit))
            })
            .map(SpotifyId)
            .ok_or_else(|| Error::Id(text.to_owned()))
    }

    pub fn from_gid(gid: &[u8]) -> Result<SpotifyId> {
        let raw: [u8; SpotifyId::GID_LEN] = gid
            .try_into()
            .map_err(|_| Error::Id(rikka_types::hex::encode(gid)))?;
        Ok(SpotifyId(u128::from_be_bytes(raw)))
    }

    #[must_use]
    pub fn to_gid(self) -> [u8; SpotifyId::GID_LEN] {
        self.0.to_be_bytes()
    }

    #[must_use]
    pub fn to_base16(self) -> String {
        rikka_types::hex::encode(self.to_gid())
    }

    #[must_use]
    pub fn to_base62(self) -> String {
        let mut digits = [BASE62[0]; BASE62_LEN];
        let mut value = self.0;
        for digit in digits.iter_mut().rev() {
            *digit = BASE62[usize::try_from(value % RADIX).unwrap_or_default()];
            value /= RADIX;
        }
        digits.iter().map(|digit| char::from(*digit)).collect()
    }
}

impl fmt::Display for SpotifyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_base62())
    }
}
