use std::any::{Any, TypeId};
use std::fmt;
use std::marker::PhantomData;

use crate::ids::SurfaceId;

pub trait Cached<S>: fmt::Debug + Sized + 'static {
    fn applied(_state: &mut S, _surface: SurfaceId) {}
}

pub(crate) type AppliedHook = &'static dyn Any;

struct Hook<S>(fn(&mut S, SurfaceId));

struct HookTable<S, T>(PhantomData<fn(S, T)>);

impl<S: 'static, T: Cached<S>> HookTable<S, T> {
    const HOOK: Hook<S> = Hook(T::applied);
}

struct Entry {
    kind: TypeId,
    value: Box<dyn Any>,
    hook: AppliedHook,
}

#[derive(Default)]
pub(crate) struct CachedStates {
    entries: Vec<Entry>,
}

impl CachedStates {
    pub(crate) fn stage<S: 'static, T: Cached<S>>(&mut self, value: T) {
        let hook: &'static Hook<S> = &HookTable::<S, T>::HOOK;
        self.put(Entry {
            kind: TypeId::of::<T>(),
            value: Box::new(value),
            hook,
        });
    }

    pub(crate) fn get<T: 'static>(&self) -> Option<&T> {
        self.entries
            .iter()
            .find(|entry| entry.kind == TypeId::of::<T>())?
            .value
            .downcast_ref::<T>()
    }

    pub(crate) fn absorb(&mut self, later: &mut CachedStates) {
        for entry in later.entries.drain(..) {
            self.put(entry);
        }
    }

    pub(crate) fn apply(
        &mut self,
        pending: &mut CachedStates,
        mut applied: impl FnMut(AppliedHook),
    ) {
        for entry in pending.entries.drain(..) {
            applied(entry.hook);
            self.put(entry);
        }
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    fn put(&mut self, entry: Entry) {
        match self.entries.iter_mut().find(|slot| slot.kind == entry.kind) {
            Some(slot) => *slot = entry,
            None => self.entries.push(entry),
        }
    }
}

impl fmt::Debug for CachedStates {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CachedStates")
            .field("entries", &self.entries.len())
            .finish()
    }
}

pub(crate) fn run<S: 'static>(state: &mut S, surface: SurfaceId, hook: AppliedHook) {
    if let Some(hook) = hook.downcast_ref::<Hook<S>>() {
        (hook.0)(state, surface);
    }
}
