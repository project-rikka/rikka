use std::path::Path;

const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;

pub(crate) struct Fnv(u64);

impl Fnv {
    pub(crate) fn new() -> Fnv {
        Fnv(OFFSET)
    }

    pub(crate) fn bytes(mut self, bytes: &[u8]) -> Fnv {
        for byte in bytes {
            self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(PRIME);
        }
        self
    }

    pub(crate) fn path(self, path: &Path) -> Fnv {
        let bytes = path.as_os_str().as_encoded_bytes();
        self.u64(bytes.len() as u64).bytes(bytes)
    }

    pub(crate) fn u64(self, value: u64) -> Fnv {
        self.bytes(&value.to_le_bytes())
    }

    pub(crate) fn finish(self) -> u64 {
        self.0
    }
}
