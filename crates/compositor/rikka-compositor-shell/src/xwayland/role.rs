use std::collections::VecDeque;

use rikka_compositor_protocols::xwayland_shell_v1::{self, XwaylandShellV1};
use rikka_compositor_protocols::xwayland_surface_v1::{self, Error, XwaylandSurfaceV1};
use rikka_compositor_surface::{Committed, PreCommit, Rejected, Role, SurfaceId, assign_role};

use crate::event::ShellEvent;
use crate::host::ShellHost;
use crate::shell::Shell;

#[derive(Debug)]
pub(crate) struct XwaylandRole {
    resource: XwaylandSurfaceV1,
    pending: Option<u64>,
    latched: VecDeque<Option<u64>>,
    serial: Option<u64>,
}

impl Shell {
    pub fn surface_for_xwayland_serial(&self, serial: u64) -> Option<SurfaceId> {
        self.xwayland.get(&serial).copied()
    }
}

pub(crate) fn shell<S: ShellHost>(
    state: &mut S,
    shell: XwaylandShellV1,
    request: xwayland_shell_v1::Request,
) {
    let xwayland_shell_v1::Request::GetXwaylandSurface { id, surface } = request else {
        return;
    };
    let Some(surface) = state.surfaces().lookup(&surface) else {
        return;
    };
    id.set_key(surface);
    if let Some(role) = state.surfaces().role_mut::<XwaylandRole>(surface)
        && !role.resource.alive()
    {
        role.resource = id;
        return;
    }
    let role = XwaylandRole {
        resource: id,
        pending: None,
        latched: VecDeque::new(),
        serial: None,
    };
    if let Err(error) = assign_role(state, surface, role) {
        error.post(&shell, xwayland_shell_v1::Error::Role.code());
    }
}

pub(crate) fn request<S: ShellHost>(
    state: &mut S,
    resource: XwaylandSurfaceV1,
    request: xwayland_surface_v1::Request,
) {
    let xwayland_surface_v1::Request::SetSerial {
        serial_lo,
        serial_hi,
    } = request
    else {
        return;
    };
    let Some(surface) = resource.key::<SurfaceId>() else {
        return;
    };
    let Some(role) = state
        .surfaces()
        .role_mut::<XwaylandRole>(surface)
        .filter(|role| role.resource == resource)
    else {
        return;
    };
    let serial = (u64::from(serial_hi) << 32) | u64::from(serial_lo);
    if serial == 0 {
        return resource.post_error(Error::InvalidSerial);
    }
    if role.serial.is_some() || role.latched.iter().any(Option::is_some) {
        return resource.post_error(Error::AlreadyAssociated);
    }
    role.pending = Some(serial);
}

impl<S: ShellHost> Role<S> for XwaylandRole {
    const NAME: &'static str = "xwayland_surface_v1";

    fn pre_commit(state: &mut S, commit: &PreCommit) -> Result<(), Rejected> {
        if let Some(role) = state.surfaces().role_mut::<XwaylandRole>(commit.surface) {
            let pending = role.pending.take();
            role.latched.push_back(pending);
        }
        Ok(())
    }

    fn committed(state: &mut S, commit: &Committed) {
        let Some(role) = state.surfaces().role_mut::<XwaylandRole>(commit.surface) else {
            return;
        };
        let Some(Some(serial)) = role.latched.pop_front() else {
            return;
        };
        role.serial = Some(serial);
        state.shell().xwayland.insert(serial, commit.surface);
        state.shell_event(ShellEvent::XwaylandAssociated {
            surface: commit.surface,
            serial,
        });
    }

    fn destroyed(state: &mut S, surface: SurfaceId, role: Self) {
        if let Some(serial) = role.serial {
            let xwayland = &mut state.shell().xwayland;
            if xwayland.get(&serial) == Some(&surface) {
                xwayland.remove(&serial);
            }
        }
    }
}
