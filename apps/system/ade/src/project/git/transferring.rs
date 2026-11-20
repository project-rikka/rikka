use ade_git::Progress;

use crate::project::git::origin::Origin;
use crate::project::git::transfer::Transfer;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Transferring {
    transfer: Transfer,
    origin: Origin,
    progress: Option<Progress>,
}

impl Transferring {
    pub(super) fn new(transfer: Transfer, origin: Origin) -> Transferring {
        Transferring {
            transfer,
            origin,
            progress: None,
        }
    }

    pub(crate) fn transfer(&self) -> Transfer {
        self.transfer
    }

    pub(crate) fn progress(&self) -> Option<Progress> {
        self.progress
    }

    pub(super) fn origin(&self) -> Origin {
        self.origin
    }

    pub(super) fn advance(&mut self, progress: Progress) {
        self.progress = Some(progress);
    }
}
