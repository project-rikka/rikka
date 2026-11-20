use std::time::Duration;

use rikka_compositor_protocols::xdg_wm_base::XdgWmBase;
use rikka_wayland_wire::ClientId;

use crate::event::ShellEvent;
use crate::host::ShellHost;
use crate::ids::WmBaseId;
use crate::shell::Shell;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PingError {
    #[error("client has not bound xdg_wm_base")]
    NoShell,
    #[error("ping {0} is still unanswered")]
    Unanswered(u32),
}

impl Shell {
    pub fn ping(&mut self, client: ClientId, patience: Duration) -> Result<u32, PingError> {
        let record = self
            .wm_bases
            .values_mut()
            .find(|record| record.resource.client() == client && record.resource.alive())
            .ok_or(PingError::NoShell)?;
        if let Some(serial) = record.ping {
            return Err(PingError::Unanswered(serial));
        }
        let serial = self.clock.serial();
        record.ping = Some(serial);
        record.resource.ping(serial);
        self.clock
            .expect_pong(patience, record.resource.clone(), serial);
        Ok(serial)
    }

    pub fn unresponsive(&self, client: ClientId) -> bool {
        self.wm_bases
            .values()
            .any(|record| record.resource.client() == client && record.overdue)
    }
}

pub(crate) fn pong<S: ShellHost>(state: &mut S, wm_base: &XdgWmBase, serial: u32) {
    let Some(record) = wm_base
        .key::<WmBaseId>()
        .and_then(|id| state.shell().wm_bases.get_mut(id))
    else {
        return;
    };
    if record.ping != Some(serial) {
        return;
    }
    record.ping = None;
    record.overdue = false;
    state.shell_event(ShellEvent::Pong {
        client: wm_base.client(),
    });
}

pub(crate) fn expired<S: ShellHost>(state: &mut S, wm_base: &XdgWmBase, serial: u32) {
    let Some(record) = wm_base
        .key::<WmBaseId>()
        .and_then(|id| state.shell().wm_bases.get_mut(id))
    else {
        return;
    };
    if record.ping != Some(serial) {
        return;
    }
    record.overdue = true;
    state.shell_event(ShellEvent::Unresponsive {
        client: wm_base.client(),
    });
}
