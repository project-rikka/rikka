use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::rc::Rc;

use crate::view::observe::{log, reads};
use crate::view::{Element, View};

pub type Setter<T> = Rc<dyn Fn(T)>;

pub(crate) type Store = Rc<dyn Any>;

pub(crate) trait Deferred {
    fn identity(&self) -> TypeId;

    fn store(&self) -> Store;

    fn resolve(self: Box<Self>, store: &Store) -> Element;
}

struct Local<T, F> {
    initial: T,
    content: F,
}

struct LocalSlot<T> {
    key: u64,
    value: RefCell<T>,
}

pub fn local_state<T, V, F>(initial: T, content: F) -> Element
where
    T: Clone + 'static,
    V: View,
    F: FnOnce(T, Setter<T>) -> V + 'static,
{
    Element::deferring(Box::new(Local { initial, content }))
}

impl<T, V, F> Deferred for Local<T, F>
where
    T: Clone + 'static,
    V: View,
    F: FnOnce(T, Setter<T>) -> V + 'static,
{
    fn identity(&self) -> TypeId {
        TypeId::of::<LocalSlot<T>>()
    }

    fn store(&self) -> Store {
        Rc::new(LocalSlot {
            key: log::key(),
            value: RefCell::new(self.initial.clone()),
        })
    }

    fn resolve(self: Box<Self>, store: &Store) -> Element {
        let slot = Rc::clone(store)
            .downcast::<LocalSlot<T>>()
            .unwrap_or_else(|_| unreachable!("local state type mismatch"));
        reads::read(slot.key);
        let value = slot.value.borrow().clone();
        let weak = Rc::downgrade(&slot);
        let set: Setter<T> = Rc::new(move |value: T| {
            if let Some(slot) = weak.upgrade() {
                *slot.value.borrow_mut() = value;
                log::changed(slot.key);
            }
        });
        (self.content)(value, set).build()
    }
}
