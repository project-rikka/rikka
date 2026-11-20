use std::num::NonZeroU32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(NonZeroU32);

impl Id {
    #[must_use]
    pub fn new(raw: u32) -> Option<Id> {
        NonZeroU32::new(raw).map(Id)
    }

    #[must_use]
    pub fn get(self) -> u32 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CloseReason {
    Expired,
    Dismissed,
    Withdrawn,
    Undefined,
}

impl CloseReason {
    pub(crate) fn from_code(code: u32) -> CloseReason {
        match code {
            1 => CloseReason::Expired,
            2 => CloseReason::Dismissed,
            3 => CloseReason::Withdrawn,
            _ => CloseReason::Undefined,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    ActionInvoked { id: Id, key: String },
    ActivationToken { id: Id, token: String },
    Closed { id: Id, reason: CloseReason },
}

impl Event {
    #[must_use]
    pub fn id(&self) -> Id {
        match self {
            Event::ActionInvoked { id, .. }
            | Event::ActivationToken { id, .. }
            | Event::Closed { id, .. } => *id,
        }
    }
}
