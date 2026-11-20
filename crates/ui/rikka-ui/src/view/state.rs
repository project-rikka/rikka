use std::any::{Any, TypeId, type_name};
use std::rc::Rc;

use crate::view::observe::log;
use crate::view::{Element, View};

pub use rikka_ui_macros::State;

pub trait State: Any {
    fn holds(&self, target: TypeId) -> usize;

    fn reach(&mut self, target: TypeId) -> Option<StateRef<'_>>;

    fn has_plain(&self) -> bool {
        true
    }
}

#[doc(hidden)]
pub struct StateRef<'a> {
    value: &'a mut dyn Any,
    plain: bool,
    logs: Option<u64>,
}

impl<'a> StateRef<'a> {
    pub fn new(value: &'a mut dyn Any, plain: bool) -> StateRef<'a> {
        StateRef {
            value,
            plain,
            logs: None,
        }
    }

    pub(crate) fn logging(self, key: u64) -> StateRef<'a> {
        StateRef {
            logs: Some(key),
            ..self
        }
    }
}

impl std::fmt::Debug for StateRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateRef")
            .field("plain", &self.plain)
            .field("logs", &self.logs)
            .finish_non_exhaustive()
    }
}

impl State for () {
    fn holds(&self, target: TypeId) -> usize {
        usize::from(TypeId::of::<Self>() == target)
    }

    fn reach(&mut self, target: TypeId) -> Option<StateRef<'_>> {
        (TypeId::of::<Self>() == target).then(|| StateRef::new(self, false))
    }

    fn has_plain(&self) -> bool {
        false
    }
}

impl<T: State> State for Option<T> {
    fn holds(&self, target: TypeId) -> usize {
        usize::from(TypeId::of::<Self>() == target)
            + self.as_ref().map_or(0, |inner| inner.holds(target))
    }

    fn reach(&mut self, target: TypeId) -> Option<StateRef<'_>> {
        if TypeId::of::<Self>() == target {
            return Some(StateRef::new(self, true));
        }
        self.as_mut()?.reach(target)
    }
}

impl<T: State> State for Box<T> {
    fn holds(&self, target: TypeId) -> usize {
        usize::from(TypeId::of::<Self>() == target) + (**self).holds(target)
    }

    fn reach(&mut self, target: TypeId) -> Option<StateRef<'_>> {
        if TypeId::of::<Self>() == target {
            let plain = (**self).has_plain();
            return Some(StateRef::new(self, plain));
        }
        (**self).reach(target)
    }

    fn has_plain(&self) -> bool {
        (**self).has_plain()
    }
}

type Lens = dyn Fn(&mut dyn Any) -> &mut dyn State;

pub(crate) struct Scoped {
    parent: TypeId,
    parent_name: &'static str,
    lens_at: usize,
    lens: Box<Lens>,
}

impl Scoped {
    pub(crate) fn same(&self, other: &Scoped) -> bool {
        self.parent == other.parent && self.lens_at == other.lens_at
    }
}

impl std::fmt::Debug for Scoped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scoped")
            .field("parent", &self.parent_name)
            .finish_non_exhaustive()
    }
}

pub fn scope<P, C, V>(lens: fn(&mut P) -> &mut C, content: V) -> Element
where
    P: 'static,
    C: State,
    V: View,
{
    let scoped = Scoped {
        parent: TypeId::of::<P>(),
        parent_name: type_name::<P>(),
        lens_at: lens as *const () as usize,
        lens: Box::new(move |parent: &mut dyn Any| -> &mut dyn State {
            lens(
                parent
                    .downcast_mut::<P>()
                    .expect("scope lens called with wrong parent type"),
            )
        }),
    };
    content.build().scoped(Rc::new(scoped))
}

pub(crate) type Chain = [Rc<Scoped>];

pub struct StateAccess<'a> {
    root: &'a mut dyn State,
    chain: &'a Chain,
}

impl std::fmt::Debug for StateAccess<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateAccess")
            .field("scopes", &self.chain.len())
            .finish_non_exhaustive()
    }
}

impl<'a> StateAccess<'a> {
    pub(crate) fn new(root: &'a mut dyn State, chain: &'a Chain) -> Self {
        Self { root, chain }
    }

    pub(crate) fn chain(&self) -> &'a Chain {
        self.chain
    }

    pub fn get<T: 'static>(&mut self) -> Option<&mut T> {
        let reached = self.locate::<T>()?;
        if reached.plain {
            crate::task::plain();
        }
        if let Some(key) = reached.logs {
            log::changed(key);
        }
        reached.value.downcast_mut::<T>()
    }

    pub(crate) fn find<T: 'static>(&mut self) -> Option<&T> {
        let _quiet = log::Quiet::enter();
        let reached = self.locate::<T>()?;
        reached.value.downcast_ref::<T>()
    }

    fn locate<T: 'static>(&mut self) -> Option<StateRef<'_>> {
        let target = TypeId::of::<T>();
        for depth in (0..=self.chain.len()).rev() {
            let matches =
                level(&mut *self.root, self.chain, depth).map_or(0, |state| state.holds(target));
            if matches == 0 {
                continue;
            }
            if matches > 1 {
                ambiguous::<T>(matches);
            }
            return level(&mut *self.root, self.chain, depth)?.reach(target);
        }
        unreachable_target::<T>();
        None
    }
}

fn level<'s>(root: &'s mut dyn State, chain: &Chain, depth: usize) -> Option<&'s mut dyn State> {
    let mut current = root;
    for scoped in &chain[..depth] {
        if current.holds(scoped.parent) == 0 {
            unreachable_scope(scoped);
            return None;
        }
        let parent = current.reach(scoped.parent)?;
        current = (scoped.lens)(parent.value);
    }
    Some(current)
}

fn ambiguous<T>(matches: usize) {
    let message = format!(
        "{matches} places hold a {}; wrap the view that acts on it in `scope` to say which",
        type_name::<T>()
    );
    if cfg!(debug_assertions) {
        panic!("{message}");
    }
    tracing::error!("{message}");
}

fn unreachable_target<T>() {
    let message = format!(
        "nothing in this window's state holds a {}; derive `State` on the struct that owns it \
         and mark nested states with `#[state]`",
        type_name::<T>()
    );
    if cfg!(debug_assertions) {
        panic!("{message}");
    }
    tracing::error!("{message}");
}

fn unreachable_scope(scoped: &Scoped) {
    let message = format!(
        "a scope expects a {} around it, and nothing holds one",
        scoped.parent_name
    );
    if cfg!(debug_assertions) {
        panic!("{message}");
    }
    tracing::error!("{message}");
}
