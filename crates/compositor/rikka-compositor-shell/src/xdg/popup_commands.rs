use rikka_compositor_geometry::{Logical, Rect};

use super::popup::PopupParent;
use crate::ids::PopupId;
use crate::shell::Shell;

impl Shell {
    pub fn configure_popup(&mut self, popup: PopupId, geometry: Rect<i32, Logical>) -> Option<u32> {
        if self.popups.get(popup).is_none_or(|record| record.dismissed) {
            return None;
        }
        let serial = self.clock.serial();
        let record = self.popups.get_mut(popup)?;
        if let Some(token) = record.reposition.take() {
            record.resource.repositioned(token);
        }
        record.resource.configure(
            geometry.loc.x,
            geometry.loc.y,
            geometry.size.w,
            geometry.size.h,
        );
        record.xdg_surface.configure(serial);
        record.lifecycle.sent(serial, geometry);
        Some(serial)
    }

    pub fn dismiss_popup(&mut self, popup: PopupId) {
        let mut doomed: Vec<(usize, PopupId)> = self
            .popups
            .keys()
            .filter_map(|candidate| {
                self.depth_below(candidate, popup)
                    .map(|depth| (depth, candidate))
            })
            .collect();
        doomed.sort_by_key(|(depth, _)| std::cmp::Reverse(*depth));
        for (_, id) in doomed {
            if let Some(record) = self.popups.get_mut(id)
                && !record.dismissed
            {
                record.dismissed = true;
                record.resource.popup_done();
            }
        }
    }

    pub fn popup_root(&self, popup: PopupId) -> Option<PopupParent> {
        let mut current = self.popups.get(popup)?.parent?;
        while let PopupParent::Popup(parent) = current {
            current = self.popups.get(parent)?.parent?;
        }
        Some(current)
    }

    pub(crate) fn dismiss_children(&mut self, parent: PopupParent) {
        let children: Vec<PopupId> = self
            .popups
            .iter()
            .filter(|(_, popup)| popup.parent == Some(parent))
            .map(|(id, _)| id)
            .collect();
        for child in children {
            self.dismiss_popup(child);
        }
    }

    fn depth_below(&self, candidate: PopupId, ancestor: PopupId) -> Option<usize> {
        let mut depth = 0;
        let mut current = candidate;
        loop {
            if current == ancestor {
                return Some(depth);
            }
            match self.popups.get(current)?.parent? {
                PopupParent::Popup(parent) => {
                    current = parent;
                    depth += 1;
                }
                PopupParent::Toplevel(_) | PopupParent::Layer(_) => return None,
            }
        }
    }
}
