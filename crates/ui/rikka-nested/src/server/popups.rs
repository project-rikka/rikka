use rikka_compositor_geometry::{Point, Rect, Size};
use rikka_compositor_seat::KeyboardFocus;
use rikka_compositor_shell::{PopupEvent, PopupId, PopupParent, ToplevelId};

use super::state::State;
use crate::view::ViewId;

impl State {
    pub(crate) fn popups_of(&self, toplevel: ToplevelId) -> Vec<PopupId> {
        self.shell
            .popups()
            .filter(|(popup, _)| match self.shell.popup_root(*popup) {
                Some(PopupParent::Toplevel(parent)) => {
                    parent == toplevel || self.root_of(parent) == Some(toplevel)
                }
                Some(PopupParent::Popup(_) | PopupParent::Layer(_)) | None => false,
            })
            .map(|(popup, _)| popup)
            .collect()
    }

    fn view_of_popup(&self, popup: PopupId) -> Option<ViewId> {
        match self.shell.popup_root(popup)? {
            PopupParent::Toplevel(toplevel) => self.owner_of(toplevel),
            PopupParent::Popup(_) | PopupParent::Layer(_) => None,
        }
    }

    pub(crate) fn popup_event(&mut self, popup: PopupId, event: PopupEvent) {
        let Some(view) = self.view_of_popup(popup) else {
            if let PopupEvent::InitialCommit | PopupEvent::Reposition { .. } = event {
                self.place_popup(None, popup);
            }
            return;
        };
        match event {
            PopupEvent::Created => {
                if let Some(record) = self.record(view)
                    && !record.popups.contains(&popup)
                {
                    record.popups.push(popup);
                }
            }
            PopupEvent::InitialCommit | PopupEvent::Reposition { .. } => {
                self.place_popup(Some(view), popup);
            }
            PopupEvent::Mapped | PopupEvent::Unmapped | PopupEvent::Configured { .. } => {
                self.mark_dirty(view);
                self.surfaces_follow(view);
            }
            PopupEvent::Destroyed => self.popup_gone(view, popup),
            PopupEvent::Grab { .. } => self.popup_grabbed(view, popup),
        }
    }

    fn place_popup(&mut self, view: Option<ViewId>, popup: PopupId) {
        let Some(state) = self.shell.popup(popup) else {
            return;
        };
        let positioner = *state.positioner();
        let parent = match state.parent() {
            Some(PopupParent::Popup(parent)) => self.popup_window_origin(parent),
            Some(PopupParent::Toplevel(toplevel)) => Some(self.toplevel_origin(toplevel)),
            Some(PopupParent::Layer(_)) | None => None,
        };
        let bounds = view
            .and_then(|view| self.views.records.get(&view))
            .and_then(|record| {
                record.desired.size.or_else(|| {
                    record
                        .toplevel
                        .and_then(|toplevel| self.window_geometry(toplevel))
                        .map(|geometry| geometry.size)
                })
            });
        let geometry = match (parent, bounds) {
            (Some(parent), Some(size)) => {
                positioner.geometry(Rect::new(Point::zero() - parent, Size::new(size.w, size.h)))
            }
            _ => positioner.placement(),
        };
        self.shell.configure_popup(popup, geometry);
        if let Some(view) = view {
            self.mark_dirty(view);
        }
    }

    fn popup_grabbed(&mut self, view: ViewId, popup: PopupId) {
        let Some(record) = self.record(view) else {
            return;
        };
        if !record.grabs.contains(&popup) {
            record.grabs.push(popup);
        }
        if self.keyboard.focused == Some(view) {
            self.refocus(view);
        }
    }

    fn popup_gone(&mut self, view: ViewId, popup: PopupId) {
        let Some(record) = self.record(view) else {
            return;
        };
        record.popups.retain(|known| *known != popup);
        let grabbed = record.grabs.contains(&popup);
        record.grabs.retain(|known| *known != popup);
        record.dirty = true;
        if grabbed && self.keyboard.focused == Some(view) {
            self.refocus(view);
        }
    }

    pub(crate) fn dismiss_grabs(&mut self, view: ViewId) {
        let first = self
            .views
            .records
            .get(&view)
            .and_then(|record| record.grabs.first().copied());
        if let Some(first) = first {
            self.shell.dismiss_popup(first);
        }
    }

    pub(crate) fn grab_contains(
        &self,
        view: ViewId,
        surface: rikka_compositor_surface::SurfaceId,
    ) -> bool {
        let Some(record) = self.views.records.get(&view) else {
            return true;
        };
        if record.grabs.is_empty() {
            return true;
        }
        let root = self.surfaces.root(surface);
        record.grabs.iter().any(|popup| {
            self.shell
                .popup(*popup)
                .is_some_and(|state| state.surface() == root)
        })
    }

    pub(crate) fn keyboard_target(&self, view: ViewId) -> Option<KeyboardFocus> {
        let record = self.views.records.get(&view)?;
        if let Some(popup) = record.grabs.last()
            && let Some(state) = self.shell.popup(*popup)
        {
            return Some(KeyboardFocus::Surface(state.surface()));
        }
        let toplevel = record.toplevel?;
        self.dependent_focus(toplevel).or_else(|| {
            Some(KeyboardFocus::Surface(
                self.shell.toplevel(toplevel)?.surface(),
            ))
        })
    }
}
