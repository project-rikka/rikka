use std::collections::BTreeSet;
use std::path::PathBuf;

use cut_project::{Media, MediaId, Project, Relink};
use rikka_app::prelude::*;

use super::hunt::Outcome;
use crate::wording;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Link {
    Missing,
    Found(Media),
    Refused { path: PathBuf, reason: Label },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Entry {
    pub(crate) media: MediaId,
    pub(crate) original: Media,
    pub(crate) uses: usize,
    pub(crate) link: Link,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Busy {
    Locating(MediaId),
    Searching,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Relinking {
    pub(crate) entries: Vec<Entry>,
    pub(crate) busy: Option<Busy>,
    pub(crate) note: Option<Label>,
}

impl Relinking {
    pub(crate) fn new(project: &Project, missing: &BTreeSet<MediaId>) -> Relinking {
        let entries = missing
            .iter()
            .filter_map(|media| {
                Some(Entry {
                    media: *media,
                    original: project.media(*media)?.clone(),
                    uses: project.uses(*media),
                    link: Link::Missing,
                })
            })
            .collect();
        Relinking {
            entries,
            busy: None,
            note: None,
        }
    }

    pub(crate) fn entry(&self, media: MediaId) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.media == media)
    }

    pub(crate) fn relinks(&self) -> Vec<Relink> {
        self.entries
            .iter()
            .filter_map(|entry| match &entry.link {
                Link::Found(to) => Some(Relink {
                    media: entry.media,
                    to: to.clone(),
                }),
                Link::Missing | Link::Refused { .. } => None,
            })
            .collect()
    }

    pub(crate) fn unresolved(&self) -> Vec<(MediaId, Media)> {
        self.entries
            .iter()
            .filter(|entry| !matches!(entry.link, Link::Found(_)))
            .map(|entry| (entry.media, entry.original.clone()))
            .collect()
    }

    pub(crate) fn settle(&mut self, outcomes: Vec<Outcome>) -> usize {
        let mut found = 0;
        for outcome in outcomes {
            let Some(entry) = self
                .entries
                .iter_mut()
                .find(|entry| entry.media == outcome.media)
            else {
                continue;
            };
            entry.link = match outcome.found {
                Ok(media) => {
                    found += 1;
                    Link::Found(media)
                }
                Err((path, refusal)) => Link::Refused {
                    reason: wording::relink_refusal(&refusal),
                    path,
                },
            };
        }
        found
    }
}
