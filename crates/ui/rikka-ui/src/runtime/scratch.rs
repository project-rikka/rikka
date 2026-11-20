use std::ops::{Deref, DerefMut};

use smallvec::SmallVec;

use crate::runtime::layout::Node;

const FEW: usize = 8;

const KEPT: usize = 32;

pub(crate) trait Pooled: Sized + 'static {
    fn pooled<R>(use_pool: impl FnOnce(&mut Vec<Vec<Self>>) -> R) -> R;
}

macro_rules! pooled {
    ($($kind:ty => $pool:ident),* $(,)?) => {
        thread_local! {
            $(static $pool: std::cell::RefCell<Vec<Vec<$kind>>> =
                const { std::cell::RefCell::new(Vec::new()) };)*
        }
        $(impl $crate::runtime::scratch::Pooled for $kind {
            fn pooled<R>(use_pool: impl FnOnce(&mut Vec<Vec<Self>>) -> R) -> R {
                $pool.with_borrow_mut(use_pool)
            }
        })*
    };
}

pub(crate) use pooled;

pooled! {
    Node => NODES,
    Option<Node> => SLOTS,
    f32 => SIZES,
    usize => BOUNDS,
    (u64, usize) => KEYED,
    (Node, f32) => WIDTHS,
    (Node, [f32; 2]) => PLACED,
}

fn recycle<T: Pooled>(mut buffer: Vec<T>) {
    buffer.clear();
    T::pooled(|pool| {
        if pool.len() < KEPT {
            pool.push(buffer);
        }
    });
}

pub(crate) struct Recycled<T: Pooled>(Vec<T>);

impl<T: Pooled> Recycled<T> {
    pub(crate) fn new() -> Self {
        Recycled(T::pooled(Vec::pop).unwrap_or_default())
    }
}

impl<T: Pooled> Drop for Recycled<T> {
    fn drop(&mut self) {
        recycle(std::mem::take(&mut self.0));
    }
}

impl<T: Pooled> Deref for Recycled<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Vec<T> {
        &self.0
    }
}

impl<T: Pooled> DerefMut for Recycled<T> {
    fn deref_mut(&mut self) -> &mut Vec<T> {
        &mut self.0
    }
}

pub(crate) struct Scratch<T: Pooled>(SmallVec<[T; FEW]>);

impl<T: Pooled> Scratch<T> {
    pub(crate) fn new() -> Self {
        Scratch(SmallVec::new())
    }

    fn with_capacity(capacity: usize) -> Self {
        if capacity <= FEW {
            return Self::new();
        }
        let mut buffer = T::pooled(Vec::pop).unwrap_or_default();
        buffer.reserve(capacity);
        Scratch(SmallVec::from_vec(buffer))
    }
}

impl<T: Pooled + Clone> Scratch<T> {
    pub(crate) fn filled(value: T, count: usize) -> Self {
        let mut scratch = Self::with_capacity(count);
        scratch.resize(count, value);
        scratch
    }
}

impl<T: Pooled> Drop for Scratch<T> {
    fn drop(&mut self) {
        if self.0.spilled() {
            recycle(std::mem::take(&mut self.0).into_vec());
        }
    }
}

impl<T: Pooled> Deref for Scratch<T> {
    type Target = SmallVec<[T; FEW]>;

    fn deref(&self) -> &SmallVec<[T; FEW]> {
        &self.0
    }
}

impl<T: Pooled> DerefMut for Scratch<T> {
    fn deref_mut(&mut self) -> &mut SmallVec<[T; FEW]> {
        &mut self.0
    }
}

impl<T: Pooled> FromIterator<T> for Scratch<T> {
    fn from_iter<I: IntoIterator<Item = T>>(items: I) -> Self {
        let items = items.into_iter();
        let (least, most) = items.size_hint();
        let mut scratch = Self::with_capacity(most.unwrap_or(least));
        scratch.extend(items);
        scratch
    }
}
