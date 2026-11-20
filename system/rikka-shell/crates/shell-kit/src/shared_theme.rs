use std::cell::RefCell;
use std::rc::Rc;

use rikka_theme::Theme;

use crate::tracked::Tracked;

type Listener = Box<dyn Fn(&Theme)>;

#[derive(Clone, Default)]
pub struct SharedTheme {
    theme: Tracked<Theme>,
    listeners: Rc<RefCell<Vec<Listener>>>,
}

impl SharedTheme {
    pub fn theme(&self) -> Theme {
        self.theme.get()
    }

    pub fn set(&self, theme: Theme) {
        if !self.theme.set(theme) {
            return;
        }
        let theme = self.theme.get();
        for listener in self.listeners.borrow().iter() {
            listener(&theme);
        }
    }

    pub fn on_change(&self, listener: impl Fn(&Theme) + 'static) {
        self.listeners.borrow_mut().push(Box::new(listener));
    }

    pub fn take_moved(&self) -> bool {
        self.theme.take_moved()
    }
}

impl std::fmt::Debug for SharedTheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedTheme")
            .field("theme", &self.theme)
            .field("listeners", &self.listeners.borrow().len())
            .finish()
    }
}
