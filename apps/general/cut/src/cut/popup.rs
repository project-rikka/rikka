use super::Cut;
use crate::easing::Picking;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Popup {
    Effects,
    Easing(Picking),
    RangeShape,
}

impl Cut {
    pub(crate) fn popup(&self) -> Option<&Popup> {
        self.session.popup.as_ref()
    }

    pub(crate) fn open_popup(&mut self, popup: Popup) {
        self.session.popup = Some(popup);
    }

    pub(crate) fn close_popup(&mut self) {
        self.session.popup = None;
    }
}
