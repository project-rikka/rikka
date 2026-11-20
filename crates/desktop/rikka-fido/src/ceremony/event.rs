use crate::webauthn::User;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewPinReason {
    NotSet,
    ChangeRequired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Event {
    Insert,
    Choose,
    Touch,
    Verify {
        retries: Option<u32>,
    },
    Pin {
        retries: u32,
        min_length: usize,
        wrong: bool,
    },
    NewPin {
        min_length: usize,
        reason: NewPinReason,
        rejected: bool,
    },
    Account {
        users: Vec<User>,
    },
}

impl Event {
    pub(crate) fn repeats(&self) -> bool {
        matches!(self, Event::Insert | Event::Choose | Event::Touch)
    }
}
