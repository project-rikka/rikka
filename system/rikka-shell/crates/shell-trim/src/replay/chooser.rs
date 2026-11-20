use std::sync::Arc;

use rikka_seam::{Request, SeamClient};
use rikka_session::replay::{BufferedSubject, Draft, ReplayStatus};
use rikka_ui::{State, skip_view, spawn_blocking};
use shell_kit::{CLOSE, Spot};

use crate::replays::{self, Error};
use crate::{Opening, opening};

pub(crate) const CAPABILITY: &str = "rikka.replay-chooser";
pub(crate) const SUMMARY: &str = "The desktop's chooser of what the instant replay kept";
pub(crate) const SUBJECT: &str = "the replay chooser";

#[derive(Clone, Debug)]
pub(crate) struct Shared {
    seam: SeamClient,
    spot: Spot,
}

impl Shared {
    #[must_use]
    pub(crate) fn new(seam: SeamClient) -> Shared {
        Shared {
            seam,
            spot: Spot::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Listing {
    Asking,
    Loaded(ReplayStatus),
    Failed(Arc<Error>),
}

#[derive(State)]
pub(crate) struct Chooser {
    shared: Shared,
    listing: Listing,
    drafting: Option<String>,
    trouble: Option<Arc<Error>>,
    selected: usize,
}

impl Chooser {
    #[must_use]
    pub(crate) fn new(shared: Shared) -> Chooser {
        Chooser {
            shared,
            listing: Listing::Asking,
            drafting: None,
            trouble: None,
            selected: 0,
        }
    }

    pub(crate) fn start(&mut self) {
        spawn_blocking(
            replays::buffer,
            |chooser: &mut Chooser, buffer: replays::Result<ReplayStatus>| {
                chooser.listed(buffer.map_err(Arc::new));
            },
        );
    }

    #[must_use]
    pub(crate) fn spot(&self) -> &Spot {
        &self.shared.spot
    }

    #[must_use]
    pub(crate) fn listing(&self) -> &Listing {
        &self.listing
    }

    #[must_use]
    pub(super) fn drafting(&self) -> Option<&str> {
        self.drafting.as_deref()
    }

    #[must_use]
    pub(crate) fn trouble(&self) -> Option<&Error> {
        self.trouble.as_deref()
    }

    #[must_use]
    pub(crate) fn selected(&self) -> usize {
        self.selected
    }

    fn subjects(&self) -> &[BufferedSubject] {
        match &self.listing {
            Listing::Loaded(buffer) => &buffer.subjects,
            Listing::Asking | Listing::Failed(_) => &[],
        }
    }

    fn listed(&mut self, buffer: Result<ReplayStatus, Arc<Error>>) {
        self.listing = match buffer {
            Ok(buffer) => Listing::Loaded(buffer),
            Err(error) => Listing::Failed(error),
        };
        let only = match self.subjects() {
            [only] => Some(only.id.clone()),
            _ => None,
        };
        if let Some(only) = only {
            self.choose(only);
        }
    }

    pub(crate) fn step(&mut self, by: isize) {
        let count = self.subjects().len();
        if count == 0 {
            return skip_view();
        }
        self.selected = (self.selected as isize + by).rem_euclid(count as isize) as usize;
    }

    pub(super) fn choose_selected(&mut self) {
        let Some(subject) = self
            .subjects()
            .get(self.selected)
            .map(|entry| entry.id.clone())
        else {
            return skip_view();
        };
        self.choose(subject);
    }

    pub(crate) fn choose(&mut self, subject: String) {
        if self.drafting.is_some() {
            return skip_view();
        }
        self.drafting = Some(subject.clone());
        self.trouble = None;
        spawn_blocking(
            move || replays::draft(&subject),
            |chooser: &mut Chooser, drafted: replays::Result<Draft>| {
                chooser.drafted(drafted.map_err(Arc::new));
            },
        );
    }

    fn drafted(&mut self, drafted: Result<Draft, Arc<Error>>) {
        self.drafting = None;
        match drafted {
            Ok(draft) => {
                self.shared
                    .seam
                    .notify(opening(&Opening::draft(draft.file)));
                self.close();
            }
            Err(error) => {
                tracing::warn!("failed to draft replay: {error}");
                self.trouble = Some(error);
            }
        }
    }

    pub(crate) fn close(&mut self) {
        self.shared.seam.notify(Request::new(CAPABILITY, CLOSE));
    }
}

impl std::fmt::Debug for Chooser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Chooser")
            .field("drafting", &self.drafting)
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}
