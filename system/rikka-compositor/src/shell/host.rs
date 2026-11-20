use rikka_compositor_shell::{Shell, ShellEvent, ShellHost, TokenRequest};

use crate::context::lock_ctx;
use crate::context::windows_ctx;
use crate::state::Compositor;

impl ShellHost for Compositor {
    fn shell(&mut self) -> &mut Shell {
        &mut self.shell
    }

    fn shell_event(&mut self, event: ShellEvent) {
        match event {
            ShellEvent::Toplevel { toplevel, event } => {
                windows_ctx!(self).toplevel_event(toplevel, event)
            }
            ShellEvent::Popup { popup, event } => windows_ctx!(self).popup_event(popup, event),
            ShellEvent::Layer { layer, event } => windows_ctx!(self).layer_event(layer, event),
            ShellEvent::Lock(event) => lock_ctx!(self).lock_event(event),
            ShellEvent::Activate { token, surface } => windows_ctx!(self).activate(token, surface),
            ShellEvent::XwaylandAssociated { surface, serial } => {
                windows_ctx!(self).xwayland_associated(surface, serial);
            }
            ShellEvent::Pong { .. } | ShellEvent::Unresponsive { .. } | ShellEvent::Bell { .. } => {
            }
        }
    }

    fn admit_activation(&mut self, request: &TokenRequest) -> bool {
        windows_ctx!(self).admits_activation(request)
    }
}
