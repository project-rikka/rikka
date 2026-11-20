use std::any::{Any, TypeId};
use std::fmt;
use std::marker::PhantomData;

use rikka_wayland_wire::Resource;

use crate::committed::{Committed, PreCommit, Rejected};
use crate::host::SurfaceHost;
use crate::ids::SurfaceId;

pub trait Role<S>: fmt::Debug + Sized + 'static {
    const NAME: &'static str;

    fn pre_commit(_state: &mut S, _commit: &PreCommit) -> Result<(), Rejected> {
        Ok(())
    }

    fn committed(_state: &mut S, _commit: &Committed) {}

    fn destroyed(_state: &mut S, _surface: SurfaceId, _role: Self) {}
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RoleError {
    #[error("surface already has role {current}, cannot assign {requested}")]
    Taken {
        current: &'static str,
        requested: &'static str,
    },
    #[error("surface already has an active {role} role object")]
    Active { role: &'static str },
    #[error("surface no longer exists")]
    Gone,
}

impl RoleError {
    pub fn post(&self, on: &impl Resource, code: u32) {
        on.object().post_error(code, &self.to_string());
    }
}

pub fn assign_role<S: SurfaceHost, R: Role<S>>(
    state: &mut S,
    surface: SurfaceId,
    role: R,
) -> Result<(), RoleError> {
    state
        .surfaces()
        .assign(surface, RoleSlot::new::<S, R>(role))
}

pub(crate) struct RoleHooks<S> {
    pub(crate) pre_commit: fn(&mut S, &PreCommit) -> Result<(), Rejected>,
    pub(crate) committed: fn(&mut S, &Committed),
    pub(crate) destroyed: fn(&mut S, SurfaceId, Box<dyn Any>),
}

impl<S> Clone for RoleHooks<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S> Copy for RoleHooks<S> {}

struct HookTable<S, R>(PhantomData<fn(S, R)>);

impl<S: 'static, R: Role<S>> HookTable<S, R> {
    const HOOKS: RoleHooks<S> = RoleHooks {
        pre_commit: R::pre_commit,
        committed: R::committed,
        destroyed: destroy_erased::<S, R>,
    };
}

fn destroy_erased<S, R: Role<S>>(state: &mut S, surface: SurfaceId, role: Box<dyn Any>) {
    if let Ok(role) = role.downcast::<R>() {
        R::destroyed(state, surface, *role);
    }
}

pub(crate) struct RoleSlot {
    name: &'static str,
    kind: TypeId,
    hooks: &'static dyn Any,
    payload: Option<Box<dyn Any>>,
}

impl RoleSlot {
    fn new<S: 'static, R: Role<S>>(role: R) -> Self {
        let hooks: &'static RoleHooks<S> = &HookTable::<S, R>::HOOKS;
        Self {
            name: R::NAME,
            kind: TypeId::of::<R>(),
            hooks,
            payload: Some(Box::new(role)),
        }
    }

    pub(crate) fn name(&self) -> &'static str {
        self.name
    }

    pub(crate) fn active(&self) -> bool {
        self.payload.is_some()
    }

    pub(crate) fn replace(&mut self, next: RoleSlot) -> Result<(), RoleError> {
        if self.kind != next.kind {
            return Err(RoleError::Taken {
                current: self.name,
                requested: next.name,
            });
        }
        if self.payload.is_some() {
            return Err(RoleError::Active { role: self.name });
        }
        self.payload = next.payload;
        Ok(())
    }

    pub(crate) fn hooks<S: 'static>(&self) -> Option<RoleHooks<S>> {
        self.payload.as_ref()?;
        self.hooks.downcast_ref::<RoleHooks<S>>().copied()
    }

    pub(crate) fn payload<R: 'static>(&self) -> Option<&R> {
        self.payload.as_ref()?.downcast_ref::<R>()
    }

    pub(crate) fn payload_mut<R: 'static>(&mut self) -> Option<&mut R> {
        self.payload.as_mut()?.downcast_mut::<R>()
    }

    pub(crate) fn end<R: 'static>(&mut self) -> Option<R> {
        if self.kind != TypeId::of::<R>() {
            return None;
        }
        self.payload.take()?.downcast::<R>().ok().map(|role| *role)
    }

    pub(crate) fn take_active<S: 'static>(&mut self) -> Option<(RoleHooks<S>, Box<dyn Any>)> {
        let hooks = self.hooks::<S>()?;
        Some((hooks, self.payload.take()?))
    }
}

impl fmt::Debug for RoleSlot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RoleSlot")
            .field("name", &self.name)
            .field("active", &self.active())
            .finish()
    }
}
