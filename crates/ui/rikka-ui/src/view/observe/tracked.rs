use std::any::{Any, TypeId};
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};

use crate::view::observe::{log, reads};
use crate::view::{State, StateRef};

pub struct Tracked<T> {
    key: u64,
    value: T,
}

impl<T> Tracked<T> {
    pub fn new(value: T) -> Tracked<T> {
        Tracked {
            key: log::key(),
            value,
        }
    }

    pub fn set(&mut self, value: T)
    where
        T: PartialEq,
    {
        if self.value != value {
            self.value = value;
            log::changed(self.key);
        }
    }

    fn read(&self) -> &T {
        reads::read(self.key);
        &self.value
    }
}

impl<T> Deref for Tracked<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.read()
    }
}

impl<T> DerefMut for Tracked<T> {
    fn deref_mut(&mut self) -> &mut T {
        log::changed(self.key);
        &mut self.value
    }
}

impl<T> Drop for Tracked<T> {
    fn drop(&mut self) {
        log::changed(self.key);
    }
}

impl<T: Clone> Clone for Tracked<T> {
    fn clone(&self) -> Self {
        Tracked::new(self.read().clone())
    }
}

impl<T: Default> Default for Tracked<T> {
    fn default() -> Self {
        Tracked::new(T::default())
    }
}

impl<T> From<T> for Tracked<T> {
    fn from(value: T) -> Self {
        Tracked::new(value)
    }
}

impl<T: PartialEq> PartialEq for Tracked<T> {
    fn eq(&self, other: &Self) -> bool {
        self.read() == other.read()
    }
}

impl<T: Eq> Eq for Tracked<T> {}

impl<T: PartialOrd> PartialOrd for Tracked<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.read().partial_cmp(other.read())
    }
}

impl<T: Ord> Ord for Tracked<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.read().cmp(other.read())
    }
}

impl<T: Hash> Hash for Tracked<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.read().hash(state);
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Tracked<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.read().fmt(f)
    }
}

impl<T: std::fmt::Display> std::fmt::Display for Tracked<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.read().fmt(f)
    }
}

impl<T: State> State for Tracked<T> {
    fn holds(&self, target: TypeId) -> usize {
        usize::from(TypeId::of::<Self>() == target) + self.value.holds(target)
    }

    fn reach(&mut self, target: TypeId) -> Option<StateRef<'_>> {
        if TypeId::of::<Self>() == target {
            return Some(StateRef::new(self as &mut dyn Any, false));
        }
        let key = self.key;
        let reached = self.value.reach(target)?;
        Some(match TypeId::of::<T>() == target {
            true => reached.logging(key),
            false => reached,
        })
    }

    fn has_plain(&self) -> bool {
        false
    }
}
