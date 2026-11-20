use crate::error::Result;
use crate::reflect::reflect;
use crate::words::words;

pub const ENTRY_POINT: &str = "main";

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shader {
    pub spirv: &'static [u8],
    pub hash: u64,
    pub uses_storage_images: bool,
    pub push_bytes: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Facts {
    pub hash: u64,
    pub uses_storage_images: bool,
    pub push_bytes: u32,
}

impl Facts {
    pub fn of(spirv: &[u8]) -> Result<Self> {
        let reflection = reflect(&words(spirv)?, ENTRY_POINT)?;
        Ok(Self {
            hash: content_hash(spirv),
            uses_storage_images: reflection.uses_storage_images(),
            push_bytes: reflection.push_size,
        })
    }

    #[must_use]
    pub fn declaration(&self, included: &str) -> String {
        format!(
            "rikka_spirv::Shader {{ spirv: include_bytes!({included:?}), hash: {:#018x}, \
             uses_storage_images: {}, push_bytes: {} }}",
            self.hash, self.uses_storage_images, self.push_bytes
        )
    }
}

#[must_use]
pub fn content_hash(spirv: &[u8]) -> u64 {
    spirv.iter().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}
