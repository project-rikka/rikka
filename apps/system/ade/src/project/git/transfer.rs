use ade_git::Status;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Transfer {
    Fetch,
    Pull,
    Push,
    Publish,
}

impl Transfer {
    pub(crate) fn from_status(status: &Status) -> Transfer {
        if status.branch().is_none() || !status.is_born() {
            return Transfer::Fetch;
        }
        if status.upstream().is_none() || status.is_upstream_gone() {
            return Transfer::Publish;
        }
        match (status.behind(), status.ahead()) {
            (0, 0) => Transfer::Fetch,
            (0, _) => Transfer::Push,
            _ => Transfer::Pull,
        }
    }

    pub(crate) fn is_exclusive(self) -> bool {
        self != Transfer::Fetch
    }
}
