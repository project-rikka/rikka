use std::ops::RangeInclusive;

use serde::Deserialize;
use serde_json::value::RawValue;

const NORMAL: u16 = 1000;
const GOING_AWAY: u16 = 1001;
const AUTHENTICATION_FAILED: u16 = 4004;
const INVALID_SEQUENCE: u16 = 4007;
const SESSION_TIMED_OUT: u16 = 4009;
const FATAL: RangeInclusive<u16> = 4010..=4014;

#[derive(Debug, Deserialize)]
pub struct Frame<'a> {
    pub op: u8,
    #[serde(borrow)]
    pub d: Option<&'a RawValue>,
    pub s: Option<u64>,
    pub t: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
pub struct Hello {
    pub heartbeat_interval: u64,
}

#[derive(Debug, Deserialize)]
pub struct ReadySession {
    pub session_id: String,
    pub resume_gateway_url: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Closing {
    Resume,
    Restart,
    SignedOut,
}

impl Closing {
    #[must_use]
    pub fn from_code(code: Option<u16>) -> Closing {
        match code {
            Some(AUTHENTICATION_FAILED) => Closing::SignedOut,
            Some(code) if FATAL.contains(&code) => Closing::SignedOut,
            Some(INVALID_SEQUENCE | SESSION_TIMED_OUT | NORMAL | GOING_AWAY) => Closing::Restart,
            _ => Closing::Resume,
        }
    }
}
