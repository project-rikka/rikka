use std::cell::RefCell;
use std::rc::Rc;

use rikka_ime::PanelAnchor;
use rikka_types::Point;

use crate::Placement;

#[derive(Clone, Debug, Default)]
pub struct AimPin(Rc<RefCell<Option<PanelAnchor>>>);

impl AimPin {
    pub fn point(&self, screen: &Placement, at: Point) {
        let at = screen.to_desktop(at);
        self.0.replace(Some(PanelAnchor::Pointer {
            output: screen.name.clone(),
            x: at.x.round() as i32,
            y: at.y.round() as i32,
        }));
    }

    pub fn clear(&self) {
        self.0.replace(None);
    }

    pub(super) fn take(&self) -> Option<PanelAnchor> {
        self.0.take()
    }
}
