use crate::code::{Algorithm, Codes, hotp};
use crate::entry::{clock, secret};
use crate::error::Result;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Totp {
    pub label: Option<String>,
    pub issuer: Option<String>,
    pub secret: String,
    pub algorithm: Algorithm,
    pub digits: u8,
    pub period: u16,
}

impl Default for Totp {
    fn default() -> Totp {
        Totp {
            label: None,
            issuer: None,
            secret: String::new(),
            algorithm: Algorithm::Sha1,
            digits: Totp::DIGITS,
            period: Totp::PERIOD,
        }
    }
}

impl Totp {
    pub const DIGITS: u8 = 6;
    pub const PERIOD: u16 = 30;

    pub fn new(secret: impl Into<String>) -> Totp {
        Totp {
            secret: secret.into(),
            ..Totp::default()
        }
    }

    pub fn secret_bytes(&self) -> Result<Vec<u8>> {
        secret::decode(&self.secret)
    }

    pub fn counter_at(&self, at: jiff::Timestamp) -> u64 {
        clock::counter(at, self.period)
    }

    pub fn remaining_at(&self, at: jiff::Timestamp) -> u16 {
        clock::remaining(at, self.period)
    }

    pub fn code_for(&self, counter: u64) -> Result<String> {
        hotp(&self.secret_bytes()?, counter, self.algorithm, self.digits)
    }

    pub fn code_at(&self, at: jiff::Timestamp) -> Result<String> {
        self.code_for(self.counter_at(at))
    }

    pub fn codes_at(&self, at: jiff::Timestamp) -> Result<Codes> {
        Codes::for_counter(self.counter_at(at), |counter| self.code_for(counter))
    }
}
