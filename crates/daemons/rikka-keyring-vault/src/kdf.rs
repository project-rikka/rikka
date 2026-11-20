use argon2::{Algorithm, Argon2, Params, Version};
use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::locked::KEY_BYTES;

pub(crate) const SALT_BYTES: usize = 16;

const MEMORY_KIB: u32 = 64 * 1024;
const PASSES: u32 = 3;
const LANES: u32 = 4;

const MEMORY_KIB_RANGE: std::ops::RangeInclusive<u32> = 8 * 1024..=1024 * 1024;
const PASSES_RANGE: std::ops::RangeInclusive<u32> = 1..=16;
const LANES_RANGE: std::ops::RangeInclusive<u32> = 1..=16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PasswordKdf {
    pub(crate) memory_kib: u32,
    pub(crate) passes: u32,
    pub(crate) lanes: u32,
    pub(crate) salt: [u8; SALT_BYTES],
}

impl PasswordKdf {
    pub(crate) fn fresh() -> Result<PasswordKdf> {
        let mut salt = [0u8; SALT_BYTES];
        getrandom::fill(&mut salt).map_err(Error::Random)?;
        Ok(PasswordKdf {
            memory_kib: MEMORY_KIB,
            passes: PASSES,
            lanes: LANES,
            salt,
        })
    }

    pub(crate) fn is_plausible(&self) -> bool {
        MEMORY_KIB_RANGE.contains(&self.memory_kib)
            && PASSES_RANGE.contains(&self.passes)
            && LANES_RANGE.contains(&self.lanes)
    }

    pub(crate) fn derive(&self, password: &[u8]) -> Result<Zeroizing<[u8; KEY_BYTES]>> {
        let params = Params::new(self.memory_kib, self.passes, self.lanes, Some(KEY_BYTES))
            .map_err(|error| Error::Kdf(error.to_string()))?;
        let mut key = Zeroizing::new([0u8; KEY_BYTES]);
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
            .hash_password_into(password, &self.salt, key.as_mut_slice())
            .map_err(|error| Error::Kdf(error.to_string()))?;
        Ok(key)
    }
}
