use crate::display::Display;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Status {
    open: Option<Display>,
    visible: bool,
}

impl Status {
    pub(crate) fn open(display: Display, visible: bool) -> Status {
        Status {
            open: Some(display),
            visible,
        }
    }

    #[must_use]
    pub fn display(self) -> Option<Display> {
        self.open
    }

    #[must_use]
    pub fn is_open(self) -> bool {
        self.open.is_some()
    }

    #[must_use]
    pub fn is_visible(self) -> bool {
        self.visible
    }
}
