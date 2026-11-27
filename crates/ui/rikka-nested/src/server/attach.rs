use std::sync::Arc;

use rikka_compositor_shell::ToplevelId;

use super::state::State;
use super::views::{Tracked, ViewRecord};
use crate::claim::{Claim, ToplevelKey};
use crate::event::Event;
use crate::frame::Frame;
use crate::look::Look;
use crate::shared::Shared;
use crate::view::ViewId;

impl State {
    pub(crate) fn adopt(&mut self, view: ViewId, claim: Claim, shared: Arc<Shared>) {
        let waiting = self.waiting_toplevel(&claim);
        let pacing = self.outputs.connect(claim.label().as_str());
        self.views
            .records
            .insert(view, ViewRecord::new(claim, pacing, shared));
        if let Some(toplevel) = waiting {
            self.attach(view, toplevel);
        }
    }

    pub(crate) fn reclaim(&mut self, view: ViewId, claim: Claim) {
        let previous = self
            .views
            .records
            .get(&view)
            .and_then(|record| record.toplevel);
        self.detach(view);
        if let Some(record) = self.record(view) {
            record.claim = claim.clone();
        }
        if let Some(previous) = previous {
            self.announce_unclaimed(previous);
        }
        if let Some(toplevel) = self.waiting_toplevel(&claim) {
            self.attach(view, toplevel);
        }
    }

    pub(crate) fn forget(&mut self, view: ViewId) {
        let toplevel = self
            .views
            .records
            .get(&view)
            .and_then(|record| record.toplevel);
        self.detach(view);
        if let Some(record) = self.views.records.remove(&view) {
            self.outputs.disconnect(record.pacing);
        }
        if let Some(toplevel) = toplevel {
            self.announce_unclaimed(toplevel);
        }
        self.describe_output();
    }

    pub(crate) fn toplevel_created(&mut self, toplevel: ToplevelId) {
        self.views.toplevels.insert(toplevel, Tracked::default());
    }

    pub(crate) fn toplevel_announced(&mut self, toplevel: ToplevelId) {
        if self.attached_view(toplevel).is_none() {
            self.claim_toplevel(toplevel);
        }
        if self.is_dependent(toplevel) || self.adopt_auxiliary(toplevel) {
            return;
        }
        self.announce_unclaimed(toplevel);
    }

    pub(crate) fn toplevel_named(&mut self, toplevel: ToplevelId) {
        let named = self
            .shell
            .toplevel(toplevel)
            .and_then(|record| record.app_id())
            .is_some_and(|app_id| !app_id.is_empty());
        if named {
            self.release_auxiliary(toplevel);
        }
        if let Some(view) = self.attached_view(toplevel) {
            let app_id = self
                .shell
                .toplevel(toplevel)
                .and_then(|record| record.app_id());
            let lost = self
                .views
                .records
                .get(&view)
                .is_some_and(|record| match &record.claim {
                    Claim::AppId(claimed) => Some(claimed.as_str()) != app_id,
                    Claim::Title(_) | Claim::Toplevel(_) => false,
                });
            if !lost {
                return;
            }
            self.detach(view);
        }
        self.toplevel_announced(toplevel);
    }

    pub(crate) fn toplevel_titled(&mut self, toplevel: ToplevelId) {
        let title = self
            .shell
            .toplevel(toplevel)
            .and_then(|record| record.title())
            .map(str::to_owned);
        match self.attached_view(toplevel) {
            Some(view) => {
                let shown = title.clone();
                self.change_look(view, |look| look.title = shown);
                if let Some(title) = title {
                    self.event_sender.send(Event::Title { view, title });
                }
            }
            None => self.toplevel_announced(toplevel),
        }
    }

    pub(crate) fn toplevel_parented(&mut self, toplevel: ToplevelId) {
        let dependent = self
            .shell
            .toplevel(toplevel)
            .is_some_and(|record| record.parent().is_some());
        if dependent && self.attached_view(toplevel).is_none() {
            self.withdraw_unclaimed(toplevel);
            self.dependent_changed(toplevel);
        }
    }

    pub(crate) fn toplevel_destroyed(&mut self, toplevel: ToplevelId) {
        let tracked = self.views.toplevels.remove(toplevel).unwrap_or_default();
        if tracked.announced {
            self.event_sender.send(Event::UnclaimedClosed {
                toplevel: ToplevelKey::of(toplevel),
            });
        }
        match tracked.view {
            Some(view) => {
                self.detach(view);
                let next = self
                    .views
                    .records
                    .get(&view)
                    .and_then(|record| match &record.claim {
                        Claim::AppId(_) => Some(record.claim.clone()),
                        Claim::Title(_) | Claim::Toplevel(_) => None,
                    });
                if let Some(claim) = next
                    && let Some(next) = self.waiting_toplevel(&claim)
                {
                    self.attach(view, next);
                }
            }
            None => {
                if let Some(owner) = tracked.owner {
                    self.owner_changed(owner);
                }
            }
        }
    }

    pub(crate) fn attached_view(&self, toplevel: ToplevelId) -> Option<ViewId> {
        self.views.toplevels.get(toplevel)?.view
    }

    fn claim_toplevel(&mut self, toplevel: ToplevelId) {
        let free = self
            .views
            .records
            .iter()
            .find(|(_, record)| record.toplevel.is_none() && self.claims(&record.claim, toplevel))
            .map(|(view, _)| *view);
        if let Some(view) = free {
            self.attach(view, toplevel);
        }
    }

    fn claims(&self, claim: &Claim, toplevel: ToplevelId) -> bool {
        let Some(record) = self.shell.toplevel(toplevel) else {
            return false;
        };
        match claim {
            Claim::AppId(app_id) => record.app_id() == Some(app_id.as_str()),
            Claim::Title(token) => {
                record.parent().is_none()
                    && record
                        .title()
                        .is_some_and(|title| title.contains(token.as_str()))
            }
            Claim::Toplevel(key) => key.toplevel() == toplevel,
        }
    }

    fn waiting_toplevel(&self, claim: &Claim) -> Option<ToplevelId> {
        self.views
            .toplevels
            .iter()
            .filter(|(_, tracked)| tracked.view.is_none())
            .map(|(toplevel, _)| toplevel)
            .filter(|toplevel| self.claims(claim, *toplevel))
            .last()
    }

    fn announce_unclaimed(&mut self, toplevel: ToplevelId) {
        let Some(record) = self.shell.toplevel(toplevel) else {
            return;
        };
        if record.parent().is_some() || !record.initial_configure_sent() {
            return;
        }
        let title = record.title().map(str::to_owned);
        let Some(tracked) = self.views.toplevels.get_mut(toplevel) else {
            return;
        };
        if tracked.view.is_some() {
            return;
        }
        tracked.announced = true;
        self.event_sender.send(Event::Unclaimed {
            toplevel: ToplevelKey::of(toplevel),
            title,
        });
    }

    fn withdraw_unclaimed(&mut self, toplevel: ToplevelId) {
        let Some(tracked) = self.views.toplevels.get_mut(toplevel) else {
            return;
        };
        if std::mem::take(&mut tracked.announced) {
            self.event_sender.send(Event::UnclaimedClosed {
                toplevel: ToplevelKey::of(toplevel),
            });
        }
    }

    fn attach(&mut self, view: ViewId, toplevel: ToplevelId) {
        self.withdraw_unclaimed(toplevel);
        let Some(tracked) = self.views.toplevels.get_mut(toplevel) else {
            return;
        };
        tracked.view = Some(view);
        let title = self
            .shell
            .toplevel(toplevel)
            .and_then(|record| record.title())
            .map(str::to_owned);
        let ready = self
            .shell
            .toplevel(toplevel)
            .is_some_and(|record| record.initial_configure_sent());
        let popups = self.popups_of(toplevel);
        let Some(record) = self.record(view) else {
            return;
        };
        record.toplevel = Some(toplevel);
        record.sent = None;
        record.dirty = true;
        record.popups = popups;
        let shared = Arc::clone(&record.shared);
        self.event_sender.send(Event::Attached { view });
        shared.change(|look| {
            look.attached = true;
            look.title = title;
        });
        if ready {
            self.configure(view, true);
        }
        self.surfaces_follow(view);
        if self.keyboard.focused == Some(view) {
            self.refocus(view);
        }
    }

    fn detach(&mut self, view: ViewId) {
        let Some(record) = self.record(view) else {
            return;
        };
        let Some(toplevel) = record.toplevel.take() else {
            return;
        };
        if let Some(gate) = record.gate.take() {
            self.display.reactor().remove(gate.patience);
        }
        let Some(record) = self.record(view) else {
            return;
        };
        record.sent = None;
        record.popups.clear();
        record.grabs.clear();
        record.placed.clear();
        record.frame_pending = false;
        record.generation += 1;
        let generation = record.generation;
        let shared = Arc::clone(&record.shared);
        self.event_sender.send(Event::Detached { view });
        shared.show(Frame {
            generation,
            ..Frame::default()
        });
        shared.change(|look| *look = Look::default());
        if let Some(tracked) = self.views.toplevels.get_mut(toplevel) {
            tracked.view = None;
        }
        self.unattached_follow(toplevel);
        if self.keyboard.focused == Some(view) {
            self.focus_keyboard(None);
        }
        if self.pointer.view == Some(view) {
            self.pointer.view = None;
        }
    }
}
