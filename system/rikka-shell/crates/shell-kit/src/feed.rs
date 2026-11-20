use std::cell::RefCell;

use rikka_ui::Observers;

#[derive(Debug, Default)]
pub struct Feed<T> {
    value: RefCell<T>,
    observers: Observers,
}

impl<T> Feed<T> {
    pub fn new(value: T) -> Feed<T> {
        Feed {
            value: RefCell::new(value),
            observers: Observers::new(),
        }
    }

    pub fn with<R>(&self, read: impl FnOnce(&T) -> R) -> R {
        self.observers.read();
        read(&self.value.borrow())
    }

    pub fn alter(&self, change: impl FnOnce(&mut T) -> bool) {
        let changed = change(&mut self.value.borrow_mut());
        if changed {
            self.observers.changed();
        }
    }
}

impl<T: Clone> Feed<T> {
    pub fn get(&self) -> T {
        self.with(T::clone)
    }
}

impl<T: PartialEq> Feed<T> {
    pub fn set(&self, value: T) {
        self.alter(|current| {
            let moved = *current != value;
            if moved {
                *current = value;
            }
            moved
        });
    }
}
