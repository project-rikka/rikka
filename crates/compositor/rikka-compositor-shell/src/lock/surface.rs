use std::collections::VecDeque;

use rikka_compositor_geometry::{Logical, Size};
use rikka_compositor_protocols::ext_session_lock_surface_v1::{
    Error, ExtSessionLockSurfaceV1, Request,
};
use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_surface::{
    BufferChange, Committed, PreCommit, Rejected, Role, Surface, SurfaceId,
};
use rikka_wayland_wire::ClientId;

use crate::configures::{AckResult, Configures};
use crate::event::{LockEvent, ShellEvent};
use crate::host::ShellHost;
use crate::ids::{LockId, LockSurfaceId};
use crate::shell::Shell;

#[derive(Debug)]
pub struct LockSurface {
    pub(crate) resource: ExtSessionLockSurfaceV1,
    pub(crate) surface: SurfaceId,
    pub(crate) lock: LockId,
    pub(crate) output: WlOutput,
    pub(crate) sent: Configures<Size<i32, Logical>>,
    pub(crate) acked: Option<Size<i32, Logical>>,
    pub(crate) latched: VecDeque<Size<i32, Logical>>,
    pub(crate) size: Option<Size<i32, Logical>>,
}

impl LockSurface {
    pub fn surface(&self) -> SurfaceId {
        self.surface
    }

    pub fn client(&self) -> ClientId {
        self.resource.client()
    }

    pub fn lock(&self) -> LockId {
        self.lock
    }

    pub fn output(&self) -> &WlOutput {
        &self.output
    }

    pub fn size(&self) -> Option<Size<i32, Logical>> {
        self.size
    }
}

#[derive(Debug)]
pub(crate) struct LockRole {
    pub(crate) id: LockSurfaceId,
}

impl Shell {
    pub fn configure_lock_surface(
        &mut self,
        surface: LockSurfaceId,
        size: Size<i32, Logical>,
    ) -> Option<u32> {
        if !self.locks.surfaces.contains_key(surface) {
            return None;
        }
        let serial = self.clock.serial();
        let record = self.locks.surfaces.get_mut(surface)?;
        record
            .resource
            .configure(serial, size.w.unsigned_abs(), size.h.unsigned_abs());
        record.sent.push(serial, size);
        Some(serial)
    }
}

pub(crate) fn request<S: ShellHost>(
    state: &mut S,
    resource: ExtSessionLockSurfaceV1,
    request: Request,
) {
    let Request::AckConfigure { serial } = request else {
        return;
    };
    let Some(id) = resource.key::<LockSurfaceId>() else {
        return;
    };
    let Some(record) = state.shell().locks.surfaces.get_mut(id) else {
        return;
    };
    match record.sent.ack(serial) {
        AckResult::Configure(size) => record.acked = Some(size),
        AckResult::Dropped | AckResult::Stale => {}
        AckResult::Unknown => resource.post_error(Error::InvalidSerial),
    }
}

pub(crate) fn destroyed<S: ShellHost>(state: &mut S, resource: ExtSessionLockSurfaceV1) {
    let Some(id) = resource.key::<LockSurfaceId>() else {
        return;
    };
    let Some(surface) = state
        .shell()
        .locks
        .surfaces
        .get(id)
        .map(|record| record.surface)
    else {
        return;
    };
    state.surfaces().end_role::<LockRole>(surface);
    remove(state, id);
}

pub(crate) fn remove<S: ShellHost>(state: &mut S, id: LockSurfaceId) {
    if state.shell().locks.surfaces.remove(id).is_some() {
        state.shell_event(ShellEvent::Lock(LockEvent::SurfaceDestroyed(id)));
    }
}

impl<S: ShellHost> Role<S> for LockRole {
    const NAME: &'static str = "ext_session_lock_surface_v1";

    fn pre_commit(state: &mut S, commit: &PreCommit) -> Result<(), Rejected> {
        let Some(id) = state
            .surfaces()
            .role::<LockRole>(commit.surface)
            .map(|role| role.id)
        else {
            return Ok(());
        };
        let has_buffer = state
            .surfaces()
            .get(commit.surface)
            .is_some_and(Surface::mapped);
        let Some(record) = state.shell().locks.surfaces.get_mut(id) else {
            return Ok(());
        };
        let error = match record.acked {
            None => Some(Error::CommitBeforeFirstAck),
            Some(_) if commit.buffer == BufferChange::Removed => Some(Error::NullBuffer),
            Some(_) if commit.buffer == BufferChange::Unchanged && !has_buffer => {
                Some(Error::NullBuffer)
            }
            Some(acked) if commit.size != Some(acked) => Some(Error::DimensionsMismatch),
            Some(_) => None,
        };
        if let Some(error) = error {
            record.resource.post_error(error);
            return Err(Rejected);
        }
        if let Some(acked) = record.acked {
            record.latched.push_back(acked);
        }
        Ok(())
    }

    fn committed(state: &mut S, commit: &Committed) {
        let Some(id) = state
            .surfaces()
            .role::<LockRole>(commit.surface)
            .map(|role| role.id)
        else {
            return;
        };
        if let Some(record) = state.shell().locks.surfaces.get_mut(id)
            && let Some(size) = record.latched.pop_front()
        {
            record.size = Some(size);
        }
    }

    fn destroyed(state: &mut S, _surface: SurfaceId, role: Self) {
        remove(state, role.id);
    }
}
