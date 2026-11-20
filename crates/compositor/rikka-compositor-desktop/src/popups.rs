use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_compositor_shell::{PopupId, PopupParent, Positioner};
use rikka_compositor_surface::SurfaceId;
use rikka_compositor_types::OutputId;

use crate::desktop::Desktop;
use crate::error::{Error, Result};
use crate::popup::{Parent, Popup};
use crate::target::Target;

impl<P> Desktop<P> {
    pub fn add_popup(
        &mut self,
        id: PopupId,
        parent: PopupParent,
        surface: SurfaceId,
    ) -> Result<()> {
        if self.popups.contains_key(id) {
            return Err(Error::DuplicatePopup(id));
        }
        let parent = self.parent(parent)?;
        self.children_mut(parent)
            .ok_or_else(|| missing(parent))?
            .push(id);
        self.popups.insert(id, Popup::new(surface, parent));
        Ok(())
    }

    pub fn remove_popup(&mut self, id: PopupId) -> Option<Popup> {
        let popup = self.popups.remove(id)?;
        if let Some(siblings) = self.children_mut(popup.parent) {
            siblings.retain(|sibling| *sibling != id);
        }
        self.drop_popups(popup.children.clone());
        Some(popup)
    }

    pub fn popup(&self, id: PopupId) -> Option<&Popup> {
        self.popups.get(id)
    }

    pub fn popup_mut(&mut self, id: PopupId) -> Option<&mut Popup> {
        self.popups.get_mut(id)
    }

    pub fn constrain_popup(
        &self,
        id: PopupId,
        positioner: &Positioner,
    ) -> Result<Rect<i32, Logical>> {
        let popup = self.popups.get(id).ok_or(Error::UnknownPopup(id))?;
        let origin = self
            .origin_of(popup.parent)
            .ok_or_else(|| missing(popup.parent))?;
        let bounds = self
            .output_of(Target::from(popup.parent))
            .and_then(|output| self.output_record(output))
            .map(|output| output.geometry());
        Ok(match bounds {
            Some(bounds) => positioner.geometry(Rect::new(bounds.loc - origin, bounds.size)),
            None => positioner.placement(),
        })
    }

    pub fn popup_location(&self, id: PopupId) -> Option<Point<i32, Logical>> {
        let popup = self.popups.get(id)?;
        Some(self.origin_of(popup.parent)? + popup.geometry.loc - popup.inset)
    }

    pub fn root_of(&self, target: Target) -> Target {
        let mut target = target;
        while let Target::Popup(popup) = target {
            match self.popups.get(popup) {
                Some(popup) => target = popup.parent.into(),
                None => break,
            }
        }
        target
    }

    pub fn output_of(&self, target: Target) -> Option<OutputId> {
        match self.root_of(target) {
            Target::Window(window) => {
                let window = self.windows.get(window)?;
                window
                    .output()
                    .or_else(|| self.home_for(window.geometry.loc))
            }
            Target::Layer(layer) => self.layers.get(layer).map(|layer| layer.output),
            Target::Lock(lock) => self.lock.surfaces.get(lock).map(|guard| guard.output),
            Target::Popup(_) => None,
        }
    }

    pub(crate) fn popups_of(&self, parent: Parent) -> &[PopupId] {
        let children = match parent {
            Parent::Window(window) => self.window_popups.get(window),
            Parent::Layer(layer) => self.layer_popups.get(layer),
            Parent::Popup(popup) => self.popups.get(popup).map(|popup| &popup.children),
        };
        children.map_or(&[], Vec::as_slice)
    }

    pub(crate) fn drop_popups(&mut self, ids: Vec<PopupId>) {
        let mut pending = ids;
        while let Some(id) = pending.pop() {
            if let Some(popup) = self.popups.remove(id) {
                pending.extend(popup.children);
            }
        }
    }

    fn parent(&self, parent: PopupParent) -> Result<Parent> {
        match parent {
            PopupParent::Toplevel(toplevel) => self
                .window_for_toplevel(toplevel)
                .map(Parent::Window)
                .ok_or(Error::UnknownToplevel(toplevel)),
            PopupParent::Layer(layer) => Ok(Parent::Layer(layer)),
            PopupParent::Popup(popup) => Ok(Parent::Popup(popup)),
        }
    }

    fn children_mut(&mut self, parent: Parent) -> Option<&mut Vec<PopupId>> {
        match parent {
            Parent::Window(window) => self.window_popups.get_mut(window),
            Parent::Layer(layer) => self.layer_popups.get_mut(layer),
            Parent::Popup(popup) => self.popups.get_mut(popup).map(|popup| &mut popup.children),
        }
    }

    pub(crate) fn origin_of(&self, parent: Parent) -> Option<Point<i32, Logical>> {
        let mut offset = Point::zero();
        let mut parent = parent;
        loop {
            match parent {
                Parent::Window(window) => {
                    return Some(offset + self.windows.get(window)?.content().loc);
                }
                Parent::Layer(layer) => return Some(offset + self.layers.get(layer)?.geometry.loc),
                Parent::Popup(popup) => {
                    let popup = self.popups.get(popup)?;
                    offset += popup.geometry.loc;
                    parent = popup.parent;
                }
            }
        }
    }
}

fn missing(parent: Parent) -> Error {
    match parent {
        Parent::Window(window) => Error::UnknownWindow(window),
        Parent::Layer(layer) => Error::UnknownLayer(layer),
        Parent::Popup(popup) => Error::UnknownPopup(popup),
    }
}
