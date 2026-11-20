use browser_pages::Cause;
use rikka_app::prelude::*;
use rikka_omnibox::{Action, Match};

use crate::omnibox::target::{Act, Target};
use crate::shell::{Shell, WindowContent, window};
use browser_model::Placement;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Opening {
    Here,
    NewTab,
    NewWindow,
}

impl Opening {
    pub(crate) fn from_modifiers(modifiers: Modifiers) -> Opening {
        match (modifiers.alt, modifiers.shift) {
            (true, _) => Opening::NewTab,
            (false, true) => Opening::NewWindow,
            (false, false) => Opening::Here,
        }
    }
}

impl Shell {
    pub(crate) fn run_match(&mut self, found: Match<Target>, opening: Opening) {
        match found.action {
            Action::Navigate(url) => self.open_address(url.into(), opening),
            Action::Search(search) => self.open_address(search.url().to_string(), opening),
            Action::SwitchToTab(target)
            | Action::RestoreArchived(target)
            | Action::RunCommand(target) => self.reach(target, opening),
        }
    }

    pub(crate) fn open_address(&mut self, address: String, opening: Opening) {
        match opening {
            Opening::Here => self.go(address, Cause::Typed),
            Opening::NewTab => {
                {
                    let mut model = self.browser().edit();
                    let opened = model.open_tab(self.id(), Some(address), Placement::Top, true);
                    if let Some(tab_entry) = opened.and_then(|tab| model.tab_mut(tab)) {
                        tab_entry.visit_tracker.set_cause(Cause::Typed);
                    }
                }
                self.changed();
            }
            Opening::NewWindow => {
                let private = self.private();
                open_window(window(
                    self.browser(),
                    private,
                    WindowContent::Addresses(vec![address]),
                ));
            }
        }
    }

    fn reach(&mut self, target: Target, opening: Opening) {
        match target {
            Target::Tab(tab) => self.select(tab),
            Target::Slot(slot, space) => {
                let here = self.model().space_of(self.id()).map(|space| space.id);
                if let Some(space) = space.filter(|space| Some(*space) != here) {
                    self.switch_space(space);
                }
                self.select_slot(slot);
            }
            Target::Archived(id) => self.restore_archived(id),
            Target::Act(act) => self.act(act, opening),
        }
    }

    pub(crate) fn act(&mut self, act: Act, opening: Opening) {
        match act {
            Act::Run(command) => self.command(command),
            Act::NewSpace => self.new_space(),
            Act::Space(space) => self.switch_space(space),
            Act::Archive => match opening {
                Opening::Here => self.go(browser_pages::Internal::Archive.address(), Cause::Typed),
                _ => self.open_internal(browser_pages::Internal::Archive),
            },
            Act::CopyAddress => {
                let active = self.active_tab();
                if let Some(tab) = active {
                    self.copy_link(tab);
                }
            }
        }
    }
}
