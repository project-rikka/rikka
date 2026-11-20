use std::sync::Arc;
use std::time::{Duration, SystemTime};

use ade_git::{Error, Progress};
use rikka_app::prelude::*;

use crate::project::act::act;
use crate::project::git::origin::Origin;
use crate::project::git::state::{Git, landing};
use crate::project::git::transfer::Transfer;
use crate::project::git::transferring::Transferring;

const FETCH_INTERVAL: Duration = Duration::from_secs(60 * 60);

const FETCH_TIMER: &str = "ade-git-fetch";

impl Git {
    pub(crate) fn remote(&self) -> Option<&str> {
        self.remote.as_deref()
    }

    pub(crate) fn transferring(&self) -> Option<Transferring> {
        self.transferring
    }

    pub(crate) fn transfer(&self) -> Transfer {
        Transfer::from_status(&self.status)
    }

    pub(crate) fn is_transferable(&self) -> bool {
        self.remote.is_some()
            && self.transferring.is_none()
            && (!self.transfer().is_exclusive() || self.operation.is_none())
    }

    pub(crate) fn sync(&mut self) {
        if !self.is_transferable() {
            skip_view();
            return;
        }
        self.start(self.transfer(), Origin::Asked);
    }

    fn fetch_in_background(&mut self) {
        self.fetch_due = None;
        if self.remote.is_none() || self.transferring.is_some() {
            skip_view();
            return;
        }
        self.start(Transfer::Fetch, Origin::Background);
    }

    fn start(&mut self, transfer: Transfer, origin: Origin) {
        let (Some(repo), Some(remote)) = (self.repo.clone(), self.remote.clone()) else {
            skip_view();
            return;
        };
        let status = self.status.clone();
        self.transferring = Some(Transferring::new(transfer, origin));
        if transfer == Transfer::Fetch {
            self.attempted = Some(SystemTime::now());
        }
        if origin == Origin::Asked {
            self.error_message = None;
        }

        let handle = self.handle.clone();
        let root = self.root.clone();
        spawn_blocking(
            move || {
                let report = |progress: Progress| {
                    let root = root.clone();
                    handle.reach(move |project| {
                        let shown = project.shown_root() == root;
                        match project.workspace(&root) {
                            Some(workspace) => workspace.git_mut().advanced(progress),
                            None => skip_view(),
                        }
                        if !shown {
                            skip_view();
                        }
                    });
                };
                match transfer {
                    Transfer::Fetch => repo.fetch(&remote, report),
                    Transfer::Pull => repo.pull(&status, report),
                    Transfer::Push | Transfer::Publish => repo.push(&status, report),
                }
            },
            landing(&self.root, Git::transferred),
        );
    }

    fn advanced(&mut self, progress: Progress) {
        match self.transferring.as_mut() {
            Some(transferring) => transferring.advance(progress),
            None => skip_view(),
        }
    }

    fn transferred(&mut self, done: ade_git::Result<()>) {
        let Some(transferring) = self.transferring.take() else {
            skip_view();
            return;
        };
        match done {
            Ok(()) => {}
            Err(error) if transferring.origin() == Origin::Background => {
                tracing::debug!(%error, path = %self.root.display(), "failed to fetch in the background");
            }
            Err(error @ Error::Rejected) => {
                self.error_message = Some(Arc::from(error.to_string()));
                self.start(Transfer::Fetch, Origin::Background);
                return;
            }
            Err(error) => self.error_message = Some(Arc::from(error.to_string())),
        }
        self.reread();
    }

    pub(super) fn schedule_fetch(&mut self) {
        let due = self.remote.as_ref().map(|_| {
            self.fetched
                .max(self.attempted)
                .map_or(SystemTime::UNIX_EPOCH, |last| last + FETCH_INTERVAL)
        });
        if due == self.fetch_due {
            return;
        }
        self.fetch_due = due;
        let timer = (FETCH_TIMER, self.root.clone());
        let Some(due) = due else {
            cancel(timer);
            return;
        };
        let root = self.root.clone();
        after_keyed(
            timer,
            due.duration_since(SystemTime::now()).unwrap_or_default(),
            act(move |project| match project.workspace(&root) {
                Some(workspace) => workspace.git_mut().fetch_in_background(),
                None => skip_view(),
            }),
        );
    }
}
