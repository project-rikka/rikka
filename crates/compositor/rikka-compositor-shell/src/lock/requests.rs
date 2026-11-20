use std::collections::VecDeque;

use rikka_compositor_protocols::ext_session_lock_manager_v1::{self, ExtSessionLockManagerV1};
use rikka_compositor_protocols::ext_session_lock_surface_v1::ExtSessionLockSurfaceV1;
use rikka_compositor_protocols::ext_session_lock_v1::{self, Error, ExtSessionLockV1};
use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_protocols::wl_surface::WlSurface;
use rikka_compositor_surface::{Surface, assign_role};

use super::session::LockObject;
use super::surface::{LockRole, LockSurface};
use crate::configures::Configures;
use crate::event::{LockEvent, ShellEvent};
use crate::host::ShellHost;
use crate::ids::LockId;

pub(crate) fn manager<S: ShellHost>(
    state: &mut S,
    _manager: ExtSessionLockManagerV1,
    request: ext_session_lock_manager_v1::Request,
) {
    let ext_session_lock_manager_v1::Request::Lock { id } = request else {
        return;
    };
    let locks = &mut state.shell().locks;
    let contested = locks.holder.is_some();
    let lock = locks.objects.insert(LockObject {
        resource: id.clone(),
        locked_sent: false,
        finished: contested,
    });
    id.set_key(lock);
    if contested {
        return id.finished();
    }
    locks.holder = Some(lock);
    emit(state, LockEvent::Requested(lock));
}

pub(crate) fn request<S: ShellHost>(
    state: &mut S,
    resource: ExtSessionLockV1,
    request: ext_session_lock_v1::Request,
) {
    let Some(lock) = resource.key::<LockId>() else {
        return;
    };
    let Some(object) = state.shell().locks.objects.get(lock) else {
        return;
    };
    let locked_sent = object.locked_sent;
    match request {
        ext_session_lock_v1::Request::Destroy => {
            if locked_sent {
                resource.post_error(Error::InvalidDestroy);
            }
        }
        ext_session_lock_v1::Request::UnlockAndDestroy => {
            if !locked_sent {
                return resource.post_error(Error::InvalidUnlock);
            }
            let locks = &mut state.shell().locks;
            if locks.holder == Some(lock) {
                locks.holder = None;
                locks.locked = false;
                emit(state, LockEvent::Unlocked(lock));
            }
        }
        ext_session_lock_v1::Request::GetLockSurface {
            id,
            surface,
            output,
        } => lock_surface(state, &resource, lock, id, &surface, output),
    }
}

pub(crate) fn destroyed<S: ShellHost>(state: &mut S, resource: ExtSessionLockV1) {
    let Some(lock) = resource.key::<LockId>() else {
        return;
    };
    let locks = &mut state.shell().locks;
    locks.objects.remove(lock);
    if locks.holder != Some(lock) {
        return;
    }
    locks.holder = None;
    let event = if locks.locked {
        LockEvent::LockerGone(lock)
    } else {
        LockEvent::Abandoned(lock)
    };
    emit(state, event);
}

fn lock_surface<S: ShellHost>(
    state: &mut S,
    lock_resource: &ExtSessionLockV1,
    lock: LockId,
    resource: ExtSessionLockSurfaceV1,
    surface: &WlSurface,
    output: WlOutput,
) {
    let Some(surface) = state.surfaces().lookup(surface) else {
        return;
    };
    if state
        .surfaces()
        .get(surface)
        .is_some_and(Surface::buffer_attached)
    {
        return lock_resource.post_error(Error::AlreadyConstructed);
    }
    let locks = &mut state.shell().locks;
    if locks
        .surfaces
        .values()
        .any(|surface| surface.lock == lock && surface.output == output)
    {
        return lock_resource.post_error(Error::DuplicateOutput);
    }
    let id = locks.surfaces.insert(LockSurface {
        resource: resource.clone(),
        surface,
        lock,
        output,
        sent: Configures::default(),
        acked: None,
        latched: VecDeque::new(),
        size: None,
    });
    if let Err(error) = assign_role(state, surface, LockRole { id }) {
        state.shell().locks.surfaces.remove(id);
        return error.post(lock_resource, Error::Role.code());
    }
    resource.set_key(id);
    emit(state, LockEvent::SurfaceCreated(id));
}

fn emit<S: ShellHost>(state: &mut S, event: LockEvent) {
    state.shell_event(ShellEvent::Lock(event));
}
