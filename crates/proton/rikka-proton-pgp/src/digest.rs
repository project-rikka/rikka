use hmac::{Hmac, KeyInit, Mac as _};
use sha1::Digest as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Hash {
    Sha1,
    Sha256,
    Sha512,
}

#[must_use]
pub fn sha256(data: impl AsRef<[u8]>) -> [u8; 32] {
    sha2::Sha256::digest(data.as_ref()).into()
}

#[must_use]
pub fn sha512(data: impl AsRef<[u8]>) -> [u8; 64] {
    sha2::Sha512::digest(data.as_ref()).into()
}

#[must_use]
pub fn hmac(hash: Hash, key: impl AsRef<[u8]>, data: impl AsRef<[u8]>) -> Vec<u8> {
    match hash {
        Hash::Sha1 => mac::<sha1::Sha1>(key.as_ref(), data.as_ref()),
        Hash::Sha256 => mac::<sha2::Sha256>(key.as_ref(), data.as_ref()),
        Hash::Sha512 => mac::<sha2::Sha512>(key.as_ref(), data.as_ref()),
    }
}

fn mac<H: hmac::EagerHash>(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac =
        <Hmac<H> as KeyInit>::new_from_slice(key).expect("HMAC accepts keys of any length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

#[derive(Debug, Clone, Default)]
pub struct Sha1(sha1::Sha1);

impl Sha1 {
    #[must_use]
    pub fn new() -> Sha1 {
        Sha1::default()
    }

    pub fn update(&mut self, data: impl AsRef<[u8]>) {
        self.0.update(data.as_ref());
    }

    #[must_use]
    pub fn finalize(self) -> [u8; 20] {
        self.0.finalize().into()
    }
}
