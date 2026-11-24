mod icon;
mod view;

use std::sync::Arc;

use discord_core::{SpaceId, SpaceTemplate};
use rikka_app::prelude::*;

pub(crate) use icon::chosen;
pub(crate) use view::{scrim, view};

const LONGEST_NAME: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Step {
    Choose,
    Customize(SpaceTemplate),
    Join,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Awaited {
    key: String,
    space: SpaceId,
}

#[derive(State)]
pub(crate) struct SpaceSheet {
    step: Tracked<Option<Step>>,
    #[untracked]
    name: TextDocument,
    #[untracked]
    invite: TextDocument,
    named: Tracked<bool>,
    invited: Tracked<bool>,
    icon: Tracked<Option<Arc<[u8]>>>,
    busy: Tracked<bool>,
    error_message: Tracked<Option<String>>,
    #[untracked]
    awaited: Option<Awaited>,
}

impl SpaceSheet {
    pub(crate) fn new() -> SpaceSheet {
        SpaceSheet {
            step: Tracked::new(None),
            name: TextDocument::new(""),
            invite: TextDocument::new(""),
            named: Tracked::new(false),
            invited: Tracked::new(false),
            icon: Tracked::new(None),
            busy: Tracked::new(false),
            error_message: Tracked::new(None),
            awaited: None,
        }
    }

    pub(crate) fn step(&self) -> Option<Step> {
        *self.step
    }

    pub(crate) fn is_open(&self) -> bool {
        self.step.is_some()
    }

    pub(crate) fn name_field(&self) -> &TextDocument {
        &self.name
    }

    pub(crate) fn invite_field(&self) -> &TextDocument {
        &self.invite
    }

    pub(crate) fn name(&self) -> String {
        self.name.text().trim().to_owned()
    }

    pub(crate) fn invite(&self) -> String {
        self.invite.text()
    }

    pub(crate) fn is_named(&self) -> bool {
        *self.named
    }

    pub(crate) fn is_invited(&self) -> bool {
        *self.invited
    }

    pub(crate) fn icon(&self) -> Option<&Arc<[u8]>> {
        self.icon.as_ref()
    }

    pub(crate) fn is_busy(&self) -> bool {
        *self.busy
    }

    pub(crate) fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub(crate) fn open(&mut self, default_name: &str) {
        self.name.reset(default_name);
        self.invite.reset("");
        self.named.set(!default_name.trim().is_empty());
        self.invited.set(false);
        self.icon.set(None);
        self.error_message.set(None);
        self.step.set(Some(Step::Choose));
    }

    pub(crate) fn close(&mut self) {
        self.step.set(None);
    }

    pub(crate) fn show(&mut self, step: Step) {
        self.error_message.set(None);
        self.step.set(Some(step));
    }

    pub(crate) fn renamed(&mut self) {
        self.named.set(!self.name().is_empty());
    }

    pub(crate) fn retyped(&mut self) {
        self.invited.set(!self.invite.text().trim().is_empty());
        self.error_message.set(None);
    }

    pub(crate) fn iconed(&mut self, picked: Result<Option<Arc<[u8]>>, String>) {
        match picked {
            Ok(Some(png)) => {
                self.icon.set(Some(png));
                self.error_message.set(None);
            }
            Ok(None) => {}
            Err(error_message) => self.error_message.set(Some(error_message)),
        }
    }

    pub(crate) fn start(&mut self) {
        self.busy.set(true);
        self.error_message.set(None);
    }

    pub(crate) fn failed(&mut self, error_message: String) {
        self.busy.set(false);
        self.error_message.set(Some(error_message));
    }

    pub(crate) fn wait_for(&mut self, key: &str, space: SpaceId) {
        self.busy.set(false);
        self.step.set(None);
        self.awaited = Some(Awaited {
            key: key.to_owned(),
            space,
        });
    }

    pub(crate) fn take_awaited(&mut self, key: &str, space: &SpaceId) -> bool {
        let matches = self
            .awaited
            .as_ref()
            .is_some_and(|awaited| awaited.key == key && awaited.space == *space);
        if matches {
            self.awaited = None;
        }
        matches
    }
}

impl std::fmt::Debug for SpaceSheet {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SpaceSheet")
            .field("step", &*self.step)
            .field("busy", &*self.busy)
            .field("error_message", &*self.error_message)
            .field("awaited", &self.awaited)
            .finish_non_exhaustive()
    }
}
