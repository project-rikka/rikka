use crate::code::{Algorithm, Codes, hotp};
use crate::entry::secret;
use crate::error::Result;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hotp {
    pub label: Option<String>,
    pub issuer: Option<String>,
    pub secret: String,
    pub algorithm: Algorithm,
    pub digits: u8,
    pub counter: u64,
}

impl Default for Hotp {
    fn default() -> Hotp {
        Hotp {
            label: None,
            issuer: None,
            secret: String::new(),
            algorithm: Algorithm::Sha1,
            digits: Hotp::DIGITS,
            counter: 0,
        }
    }
}

impl Hotp {
    pub const DIGITS: u8 = 6;

    pub fn new(secret: impl Into<String>) -> Hotp {
        Hotp {
            secret: secret.into(),
            ..Hotp::default()
        }
    }

    pub fn secret_bytes(&self) -> Result<Vec<u8>> {
        secret::decode(&self.secret)
    }

    pub fn code_for(&self, counter: u64) -> Result<String> {
        hotp(&self.secret_bytes()?, counter, self.algorithm, self.digits)
    }

    pub fn code(&self) -> Result<String> {
        self.code_for(self.counter)
    }

    pub fn codes(&self) -> Result<Codes> {
        Codes::for_counter(self.counter, |counter| self.code_for(counter))
    }
}
