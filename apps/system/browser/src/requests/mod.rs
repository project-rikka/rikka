mod dialog;
mod dispatch;
mod dropdowns;
mod files;
mod menus;
mod permissions;
mod prompts;
mod sharing;
mod sheets;
mod signing;
mod view;

pub(crate) use dropdowns::{DropdownOwner, DropdownSlot, view as dropdown_view};
pub(crate) use permissions::{decided_here, prompt};
pub(crate) use sharing::{share, share_all};
pub(crate) use sheets::{framed as framed_card, scrimmed};
pub(crate) use view::{Mark, framed};

use browser_model::{Ask, Requester};
use std::collections::HashMap;

use rikka_app::prelude::*;
use rikka_web::{Choose, ContextMenu, MediaRequest, PermissionRequest};

use crate::find::Find;
use crate::interstitial::Failure;
use crate::requests::dialog::{Dialog, Dialogs};
use crate::requests::signing::Signing;
use browser_model::TabId;

#[derive(Debug)]
pub(crate) enum Asking {
    Content(PermissionRequest),
    Media(MediaRequest),
}

impl Asking {
    pub(crate) fn allow(self) {
        match self {
            Asking::Content(request) => request.allow(),
            Asking::Media(request) => {
                let video = request.video.first().cloned();
                let audio = request.audio.first().cloned();
                request.grant(video.as_ref(), audio.as_ref());
            }
        }
    }

    pub(crate) fn deny(self) {
        match self {
            Asking::Content(request) => request.deny(),
            Asking::Media(request) => request.deny(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct PermissionPrompt {
    pub(crate) request: Asking,
    pub(crate) asks: Vec<Ask>,
    pub(crate) requester: Requester,
    pub(crate) site: String,
}

#[derive(Debug)]
pub(crate) struct ContextMenuRequest {
    pub(crate) menu: ContextMenu,
    pub(crate) selected: Option<String>,
    pub(crate) opened: u64,
}

#[derive(Debug)]
pub(crate) struct ChoiceRequest {
    pub(crate) prompt: Choose,
    pub(crate) chosen: Vec<String>,
    pub(crate) opened: u64,
}

#[derive(Debug, Default)]
pub(crate) struct TabRequests {
    pub(crate) dialogs: Dialogs,
    pub(crate) answer: TextDocument,
    pub(crate) checked: bool,
    pub(crate) prompts: std::collections::VecDeque<PermissionPrompt>,
    pub(crate) signing: Option<Signing>,
    pub(crate) pointing: Option<ContextMenuRequest>,
    pub(crate) choosing: Option<ChoiceRequest>,
    pub(crate) dropdown: DropdownSlot,
    pub(crate) failure: Failure,
    pub(crate) find: Find,
    pub(crate) picking_files: bool,
}

impl TabRequests {
    pub(crate) fn holds_page_focus(&self) -> bool {
        self.dropdown.is_open() || self.dialogs.current().is_some_and(Dialog::picks_for_field)
    }
}

#[derive(Debug, Default)]
pub(crate) struct Requests {
    tabs: HashMap<TabId, TabRequests>,
    opened: u64,
    revision: Revision,
}

impl Requests {
    pub(crate) fn get(&self, tab: TabId) -> Option<&TabRequests> {
        self.revision.read();
        self.tabs.get(&tab)
    }

    pub(crate) fn edit(&mut self, tab: TabId) -> &mut TabRequests {
        self.revision.changed();
        self.tabs.entry(tab).or_default()
    }

    pub(crate) fn edit_existing(&mut self, tab: TabId) -> Option<&mut TabRequests> {
        let tab_entry = self.tabs.get_mut(&tab)?;
        self.revision.changed();
        Some(tab_entry)
    }

    pub(crate) fn quietly(&mut self, tab: TabId) -> Option<&mut TabRequests> {
        self.tabs.get_mut(&tab)
    }

    pub(crate) fn next_opening(&mut self) -> u64 {
        self.opened += 1;
        self.opened
    }

    pub(crate) fn release(&mut self, gone: impl Fn(TabId) -> bool) -> Vec<TabRequests> {
        let leaving: Vec<TabId> = self.tabs.keys().copied().filter(|tab| gone(*tab)).collect();
        if leaving.is_empty() {
            return Vec::new();
        }
        self.revision.changed();
        leaving
            .into_iter()
            .filter_map(|tab| self.tabs.remove(&tab))
            .collect()
    }
}
