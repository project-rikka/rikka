use crate::code::{Codes, steam};
use crate::entry::{clock, secret};
use crate::error::Result;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Steam {
    pub secret: String,
    pub name: Option<String>,
}

impl Steam {
    pub const DIGITS: u8 = steam::DIGITS;
    pub const ISSUER: &'static str = steam::ISSUER;
    pub const PERIOD: u16 = steam::PERIOD;

    pub fn new(secret: impl Into<String>) -> Steam {
        Steam {
            secret: secret.into(),
            name: None,
        }
    }

    pub fn secret_bytes(&self) -> Result<Vec<u8>> {
        secret::decode_steam(&self.secret)
    }

    pub fn counter_at(&self, at: jiff::Timestamp) -> u64 {
        clock::counter(at, Steam::PERIOD)
    }

    pub fn remaining_at(&self, at: jiff::Timestamp) -> u16 {
        clock::remaining(at, Steam::PERIOD)
    }

    pub fn code_for(&self, counter: u64) -> Result<String> {
        Ok(steam::code(&self.secret_bytes()?, counter))
    }

    pub fn code_at(&self, at: jiff::Timestamp) -> Result<String> {
        self.code_for(self.counter_at(at))
    }

    pub fn codes_at(&self, at: jiff::Timestamp) -> Result<Codes> {
        Codes::for_counter(self.counter_at(at), |counter| self.code_for(counter))
    }
}
