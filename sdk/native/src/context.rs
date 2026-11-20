use std::rc::Rc;
use std::sync::Arc;

use crate::window::{Dialogs, Window};

#[derive(Debug)]
pub(crate) struct Context {
    window: Arc<Window>,
    dialogs: Rc<Dialogs>,
}

impl Context {
    pub(crate) fn new(window: Arc<Window>, dialogs: Rc<Dialogs>) -> Self {
        Self { window, dialogs }
    }

    pub(crate) fn window(&self) -> &Window {
        &self.window
    }

    pub(crate) fn dialogs(&self) -> &Dialogs {
        &self.dialogs
    }
}
