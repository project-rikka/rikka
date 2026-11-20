use std::str::FromStr;

use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Algorithm {
    #[default]
    Sha1,
    Sha256,
    Sha512,
}

impl Algorithm {
    pub fn as_str(&self) -> &'static str {
        match self {
            Algorithm::Sha1 => "SHA1",
            Algorithm::Sha256 => "SHA256",
            Algorithm::Sha512 => "SHA512",
        }
    }
}

impl FromStr for Algorithm {
    type Err = Error;

    fn from_str(name: &str) -> Result<Algorithm> {
        match name.to_ascii_uppercase().as_str() {
            "SHA1" => Ok(Algorithm::Sha1),
            "SHA256" => Ok(Algorithm::Sha256),
            "SHA512" => Ok(Algorithm::Sha512),
            _ => Err(Error::UnsupportedAlgorithm {
                name: name.to_owned(),
            }),
        }
    }
}

impl std::fmt::Display for Algorithm {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}
