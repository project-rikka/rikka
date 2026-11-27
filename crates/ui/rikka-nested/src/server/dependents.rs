use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_seat::KeyboardFocus;
use rikka_compositor_shell::{DecorationMode, ToplevelConfigure, ToplevelId, ToplevelStates};

use super::state::State;
use crate::view::ViewId;

const ANCESTRY: usize = 16;

const AUXILIARY_MARGIN: i32 = 8;

impl State {
    pub(crate) fn owner_of(&self, toplevel: ToplevelId) -> Option<ViewId> {
        let root = self.root_of(toplevel)?;
        self.attached_view(root)
    }

    pub(crate) fn root_of(&self, toplevel: ToplevelId) -> Option<ToplevelId> {
        let mut current = toplevel;
        for _ in 0..ANCESTRY {
            if self.attached_view(current).is_some() {
                return Some(current);
            }
            current = match self.shell.toplevel(current)?.parent() {
                Some(parent) => parent,
                None => self.views.toplevels.get(current)?.adopted?,
            };
        }
        None
    }

    pub(crate) fn dependents_of(&self, root: ToplevelId) -> Vec<ToplevelId> {
        self.shell
            .toplevels()
            .filter(|(toplevel, record)| {
                *toplevel != root
                    && record.mapped()
                    && self.is_dependent(*toplevel)
                    && self.root_of(*toplevel) == Some(root)
            })
            .map(|(toplevel, _)| toplevel)
            .collect()
    }

    pub(crate) fn toplevel_origin(&self, toplevel: ToplevelId) -> Point<i32, Logical> {
        let Some(root) = self.root_of(toplevel).filter(|root| *root != toplevel) else {
            return Point::zero();
        };
        let (Some(within), Some(own)) =
            (self.window_geometry(root), self.window_geometry(toplevel))
        else {
            return Point::zero();
        };
        match self.is_auxiliary(toplevel) {
            true => Point::new(
                (within.size.w - own.size.w - AUXILIARY_MARGIN).max(0),
                AUXILIARY_MARGIN,
            ),
            false => Point::new(
                ((within.size.w - own.size.w) / 2).max(0),
                ((within.size.h - own.size.h) / 2).max(0),
            ),
        }
    }

    pub(crate) fn configure_dependent(&mut self, toplevel: ToplevelId) {
        self.shell.configure_toplevel(
            toplevel,
            ToplevelConfigure {
                size: None,
                states: ToplevelStates::ACTIVATED,
                bounds: None,
                capabilities: None,
                decoration: Some(DecorationMode::ServerSide),
            },
        );
    }

    pub(crate) fn is_dependent(&self, toplevel: ToplevelId) -> bool {
        self.attached_view(toplevel).is_none()
            && (self
                .shell
                .toplevel(toplevel)
                .is_some_and(|record| record.parent().is_some())
                || self.is_auxiliary(toplevel))
    }

    fn is_auxiliary(&self, toplevel: ToplevelId) -> bool {
        self.views
            .toplevels
            .get(toplevel)
            .is_some_and(|tracked| tracked.adopted.is_some())
    }

    pub(crate) fn adopt_auxiliary(&mut self, toplevel: ToplevelId) -> bool {
        let Some(record) = self.shell.toplevel(toplevel) else {
            return false;
        };
        let anonymous = record.app_id().is_none_or(str::is_empty);
        if !anonymous || record.parent().is_some() || self.attached_view(toplevel).is_some() {
            return false;
        }
        let owner = self
            .keyboard
            .focused
            .or(self.keyboard.last_focused)
            .and_then(|view| self.views.records.get(&view))
            .and_then(|record| record.toplevel);
        let Some(owner) = owner else {
            return false;
        };
        let Some(tracked) = self.views.toplevels.get_mut(toplevel) else {
            return false;
        };
        tracked.adopted = Some(owner);
        let configured = self
            .shell
            .toplevel(toplevel)
            .is_some_and(|record| record.initial_configure_sent());
        if configured {
            self.configure_dependent(toplevel);
        }
        self.dependent_changed(toplevel);
        true
    }

    pub(crate) fn release_auxiliary(&mut self, toplevel: ToplevelId) {
        let Some(tracked) = self.views.toplevels.get_mut(toplevel) else {
            return;
        };
        if tracked.adopted.take().is_none() {
            return;
        }
        if let Some(owner) = tracked.owner.take() {
            self.owner_changed(owner);
        }
    }

    pub(crate) fn dependent_changed(&mut self, toplevel: ToplevelId) {
        let owner = self.owner_of(toplevel);
        if let Some(tracked) = self.views.toplevels.get_mut(toplevel) {
            tracked.owner = owner;
        }
        if let Some(owner) = owner {
            self.owner_changed(owner);
        }
    }

    pub(crate) fn owner_changed(&mut self, view: ViewId) {
        self.mark_dirty(view);
        self.surfaces_follow(view);
        if self.keyboard.focused == Some(view) {
            self.refocus(view);
        }
    }

    pub(crate) fn dependent_focus(&self, root: ToplevelId) -> Option<KeyboardFocus> {
        let dependent = self.dependents_of(root).pop()?;
        let surface = self.shell.toplevel(dependent)?.surface();
        Some(KeyboardFocus::Surface(surface))
    }
}
