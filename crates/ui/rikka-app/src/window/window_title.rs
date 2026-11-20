use std::cell::RefCell;
use std::rc::Rc;

use rikka_ui::Label;

#[derive(Clone, Debug, Default)]
pub(crate) struct WindowTitle(Rc<RefCell<TitleState>>);

#[derive(Debug, Default)]
struct TitleState {
    last: Option<Label>,
    unread: bool,
}

impl WindowTitle {
    pub(crate) fn set(&self, title: Label) {
        let mut state = self.0.borrow_mut();
        if state.last.as_ref() == Some(&title) {
            return;
        }
        state.last = Some(title);
        state.unread = true;
    }

    #[must_use]
    pub(crate) fn take(&self) -> Option<Label> {
        let mut state = self.0.borrow_mut();
        std::mem::take(&mut state.unread)
            .then(|| state.last.clone())
            .flatten()
    }
}
