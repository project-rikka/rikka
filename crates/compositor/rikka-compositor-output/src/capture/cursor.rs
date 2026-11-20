use super::store::Captures;
use super::types::CursorPlacement;
use crate::ids::CursorSessionId;

impl<T: Copy + PartialEq> Captures<T> {
    pub fn place_cursor(&mut self, cursor: CursorSessionId, placement: Option<CursorPlacement>) {
        let Some(entry) = self.cursors.get_mut(cursor) else {
            return;
        };
        let previous = std::mem::replace(&mut entry.placement, placement);
        let resource = &entry.resource;
        match (previous, placement) {
            (None, None) => {}
            (Some(_), None) => resource.leave(),
            (None, Some(now)) => {
                resource.enter();
                resource.position(now.position.x, now.position.y);
                resource.hotspot(now.hotspot.x, now.hotspot.y);
            }
            (Some(before), Some(now)) => {
                if before.position != now.position {
                    resource.position(now.position.x, now.position.y);
                }
                if before.hotspot != now.hotspot {
                    resource.hotspot(now.hotspot.x, now.hotspot.y);
                }
            }
        }
    }
}
