#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadyState {
    Open,
    Connecting,
    Closed,
}

impl ReadyState {
    pub(crate) fn new(closed: bool, connected: bool) -> ReadyState {
        match (closed, connected) {
            (true, _) => ReadyState::Closed,
            (false, true) => ReadyState::Open,
            (false, false) => ReadyState::Connecting,
        }
    }
}
