#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseReason {
    Expired,
    Dismissed,
    Withdrawn,
    Undefined,
}

impl CloseReason {
    #[must_use]
    pub(crate) fn code(self) -> u32 {
        match self {
            CloseReason::Expired => 1,
            CloseReason::Dismissed => 2,
            CloseReason::Withdrawn => 3,
            CloseReason::Undefined => 4,
        }
    }
}
