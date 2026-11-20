use std::sync::Arc;

use rikka_compositor_shell::ToplevelId;

use super::state::State;
use super::views::{Tracked, ViewRecord};
use crate::event::Event;
use crate::frame::Frame;
use crate::look::Look;
use crate::shared::Shared;
use crate::view::ViewId;

impl State {
    pub(crate) fn adopt(&mut self, view: ViewId, app_id: String, shared: Arc<Shared>) {
        let waiting = self.waiting_toplevel(&app_id);
        let pacing = self.outputs.connect(app_id.as_str());
        self.views
            .records
            .insert(view, ViewRecord::new(app_id, pacing, shared));
        if let Some(toplevel) = waiting {
            self.attach(view, toplevel);
        }
    }

    pub(crate) fn forget(&mut self, view: ViewId) {
        self.detach(view);
        if let Some(record) = self.views.records.remove(&view) {
            self.outputs.disconnect(record.pacing);
        }
        self.describe_output();
    }

    pub(crate) fn toplevel_created(&mut self, toplevel: ToplevelId) {
        self.views.toplevels.insert(toplevel, Tracked::default());
    }

    pub(crate) fn toplevel_named(&mut self, toplevel: ToplevelId) {
        let app_id = self
            .shell
            .toplevel(toplevel)
            .and_then(|record| record.app_id())
            .map(str::to_owned);
        let current = self
            .views
            .toplevels
            .get(toplevel)
            .and_then(|tracked| tracked.view);
        if let Some(view) = current {
            let same = self
                .views
                .records
                .get(&view)
                .is_some_and(|record| Some(record.app_id.as_str()) == app_id.as_deref());
            if same {
                return;
            }
            self.detach(view);
        }
        let Some(app_id) = app_id else {
            return;
        };
        let free = self
            .views
            .records
            .iter()
            .find(|(_, record)| record.app_id == app_id && record.toplevel.is_none())
            .map(|(view, _)| *view);
        if let Some(view) = free {
            self.attach(view, toplevel);
        }
    }

    pub(crate) fn toplevel_destroyed(&mut self, toplevel: ToplevelId) {
        let view = self
            .views
            .toplevels
            .remove(toplevel)
            .and_then(|tracked| tracked.view);
        if let Some(view) = view {
            self.detach(view);
            if let Some(app_id) = self.record(view).map(|record| record.app_id.clone())
                && let Some(next) = self.waiting_toplevel(&app_id)
            {
                self.attach(view, next);
            }
        }
    }

    fn waiting_toplevel(&self, app_id: &str) -> Option<ToplevelId> {
        self.views
            .toplevels
            .iter()
            .filter(|(_, tracked)| tracked.view.is_none())
            .map(|(toplevel, _)| toplevel)
            .filter(|toplevel| {
                self.shell
                    .toplevel(*toplevel)
                    .and_then(|record| record.app_id())
                    == Some(app_id)
            })
            .last()
    }

    fn attach(&mut self, view: ViewId, toplevel: ToplevelId) {
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
