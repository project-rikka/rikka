use crate::kind::SelectionKind;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("unknown seat")]
    UnknownSeat,
    #[error("seat has no {kind} selection")]
    NoSelection { kind: SelectionKind },
    #[error("{kind} selection does not offer {mime_type}")]
    Unoffered {
        kind: SelectionKind,
        mime_type: String,
    },
    #[error("no drag for this offer")]
    NoDrag,
    #[error("seat already has an active drag")]
    Dragging,
}

pub type Result<T> = std::result::Result<T, Error>;
