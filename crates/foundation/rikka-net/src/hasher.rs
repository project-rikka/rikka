use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};

use crate::checksum::ChecksumAlgorithm;

#[derive(Debug, Clone)]
pub(crate) enum Hasher {
    Sha1(Sha1),
    Sha256(Sha256),
    Sha512(Sha512),
}

impl Hasher {
    pub(crate) fn new(algorithm: ChecksumAlgorithm) -> Hasher {
        match algorithm {
            ChecksumAlgorithm::Sha1 => Hasher::Sha1(Sha1::new()),
            ChecksumAlgorithm::Sha256 => Hasher::Sha256(Sha256::new()),
            ChecksumAlgorithm::Sha512 => Hasher::Sha512(Sha512::new()),
        }
    }

    pub(crate) fn update(&mut self, bytes: &[u8]) {
        match self {
            Hasher::Sha1(state) => state.update(bytes),
            Hasher::Sha256(state) => state.update(bytes),
            Hasher::Sha512(state) => state.update(bytes),
        }
    }

    pub(crate) fn finish(self) -> Vec<u8> {
        match self {
            Hasher::Sha1(state) => state.finalize().to_vec(),
            Hasher::Sha256(state) => state.finalize().to_vec(),
            Hasher::Sha512(state) => state.finalize().to_vec(),
        }
    }
}
