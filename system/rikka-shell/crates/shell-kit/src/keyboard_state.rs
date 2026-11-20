use std::cell::RefCell;
use std::rc::Rc;

use rikka_types::KeyboardState as Typing;
use rikka_ui::Observers;

#[derive(Clone, Default)]
pub struct KeyboardState(Rc<Inner>);

#[derive(Default)]
struct Inner {
    typing: RefCell<Typing>,
    observers: Observers,
}

impl KeyboardState {
    #[must_use]
    pub fn typing(&self) -> Typing {
        self.0.observers.read();
        self.0.typing.borrow().clone()
    }

    pub fn settle(&self, typing: Typing) {
        if *self.0.typing.borrow() == typing {
            return;
        }
        *self.0.typing.borrow_mut() = typing;
        self.0.observers.changed();
    }
}

impl std::fmt::Debug for KeyboardState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("KeyboardState")
            .field(&*self.0.typing.borrow())
            .finish()
    }
}
