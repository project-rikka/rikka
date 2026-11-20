use crate::error::Error;

const FIRST_VOICE_CODE: u16 = 4000;
const AUTHENTICATION_FAILED: u16 = 4004;
const SESSION_INVALID: u16 = 4006;
const DISCONNECTED: u16 = 4014;
const SERVER_CRASHED: u16 = 4015;
const CALL_TERMINATED: u16 = 4022;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Closing {
    Resume,
    Finished,
    Failed(Error),
}

pub(crate) fn closing(code: Option<u16>, reason: &str) -> Closing {
    match code {
        None => Closing::Resume,
        Some(code) if code < FIRST_VOICE_CODE || code == SERVER_CRASHED => Closing::Resume,
        Some(DISCONNECTED | CALL_TERMINATED) => Closing::Finished,
        Some(SESSION_INVALID) => Closing::Failed(Error::SessionInvalid),
        Some(AUTHENTICATION_FAILED) => Closing::Failed(Error::Rejected),
        Some(code) => Closing::Failed(Error::Closed {
            code,
            reason: reason.to_owned(),
        }),
    }
}
