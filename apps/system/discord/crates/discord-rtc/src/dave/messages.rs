#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Input {
    ExternalSender(Vec<u8>),
    Proposals { revoke: bool, proposals: Vec<u8> },
    Commit { transition: u16, commit: Vec<u8> },
    Welcome { transition: u16, welcome: Vec<u8> },
    PrepareTransition { transition: u16, version: u16 },
    ExecuteTransition { transition: u16 },
    PrepareEpoch { version: u16, epoch: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Reply {
    KeyPackage(Vec<u8>),
    CommitWelcome {
        commit: Vec<u8>,
        welcome: Option<Vec<u8>>,
    },
    TransitionReady(u16),
    InvalidCommitWelcome(u16),
}
