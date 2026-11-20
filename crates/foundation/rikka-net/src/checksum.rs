use std::fmt;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ChecksumAlgorithm {
    Sha256,
    Sha512,
}

impl ChecksumAlgorithm {
    #[must_use]
    pub const fn digest_len(self) -> usize {
        match self {
            ChecksumAlgorithm::Sha256 => 32,
            ChecksumAlgorithm::Sha512 => 64,
        }
    }
}

impl fmt::Display for ChecksumAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            ChecksumAlgorithm::Sha256 => "SHA-256",
            ChecksumAlgorithm::Sha512 => "SHA-512",
        };
        f.write_str(name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Checksum {
    algorithm: ChecksumAlgorithm,
    digest: Vec<u8>,
}

impl Checksum {
    pub fn sha256(digest: &str) -> Result<Checksum> {
        Checksum::from_hex(ChecksumAlgorithm::Sha256, digest)
    }

    pub fn sha512(digest: &str) -> Result<Checksum> {
        Checksum::from_hex(ChecksumAlgorithm::Sha512, digest)
    }

    pub fn new(algorithm: ChecksumAlgorithm, digest: impl Into<Vec<u8>>) -> Result<Checksum> {
        let digest = digest.into();
        if digest.len() != algorithm.digest_len() {
            return Err(Error::InvalidChecksum(format!(
                "a {algorithm} digest is {} bytes, not {}",
                algorithm.digest_len(),
                digest.len()
            )));
        }
        Ok(Checksum { algorithm, digest })
    }

    fn from_hex(algorithm: ChecksumAlgorithm, digest: &str) -> Result<Checksum> {
        let bytes = rikka_types::hex::decode(digest.trim()).map_err(|error| {
            Error::InvalidChecksum(format!("invalid hex digest \"{digest}\": {error}"))
        })?;
        Checksum::new(algorithm, bytes)
    }

    #[must_use]
    pub fn algorithm(&self) -> ChecksumAlgorithm {
        self.algorithm
    }

    #[must_use]
    pub fn digest(&self) -> &[u8] {
        &self.digest
    }

    #[must_use]
    pub fn to_hex(&self) -> String {
        rikka_types::hex::encode(&self.digest)
    }

    pub(crate) fn verify(&self, actual: &[u8]) -> Result<()> {
        if self.digest == actual {
            return Ok(());
        }
        Err(Error::ChecksumMismatch {
            algorithm: self.algorithm,
            expected: self.to_hex(),
            actual: rikka_types::hex::encode(actual),
        })
    }
}

impl fmt::Display for Checksum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.algorithm, self.to_hex())
    }
}
