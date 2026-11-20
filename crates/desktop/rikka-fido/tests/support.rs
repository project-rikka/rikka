use rikka_fido::{Ceremony, Event, Result};
use sha2::{Digest as _, Sha256};

pub(crate) const ORIGIN: &str = "https://account.example.com";
pub(crate) const RP: &str = "example.com";
pub(crate) const CHALLENGE: [u8; 32] = [0x5a; 32];

pub(crate) struct Answers<'a> {
    pub(crate) pins: &'a [&'a str],
    pub(crate) account: usize,
}

pub(crate) const SILENT: Answers<'static> = Answers {
    pins: &[],
    account: 0,
};

pub(crate) fn drive<T>(mut ceremony: Ceremony<T>, answers: Answers<'_>) -> (Result<T>, Vec<Event>) {
    let mut pins = answers.pins.iter();
    let mut events = Vec::new();

    while let Some(event) = ceremony.blocking_event() {
        match &event {
            Event::Pin { .. } | Event::NewPin { .. } => match pins.next() {
                Some(pin) => ceremony.pin(*pin),
                None => ceremony.cancel(),
            },
            Event::Account { .. } => ceremony.choose(answers.account),
            _ => {}
        }
        events.push(event);
    }

    (ceremony.wait(), events)
}

pub(crate) fn sha256(data: impl AsRef<[u8]>) -> [u8; 32] {
    Sha256::digest(data.as_ref()).into()
}

pub(crate) fn credential(seed: u8, length: usize) -> Vec<u8> {
    (0..length).map(|at| seed.wrapping_add(at as u8)).collect()
}
