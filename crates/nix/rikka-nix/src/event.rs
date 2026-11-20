use rikka_nix_protocol::{Run, RunId, Status};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Started(Run),
    Lines(Lines),
    Status { run: RunId, status: Status },
    Finished(Run),
    Exited,
    Disconnected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lines {
    pub run: RunId,
    pub from: u64,
    pub lines: Vec<String>,
}
