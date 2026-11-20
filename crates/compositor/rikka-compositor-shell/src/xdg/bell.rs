use rikka_compositor_protocols::xdg_system_bell_v1::{Request, XdgSystemBellV1};

use crate::event::ShellEvent;
use crate::host::ShellHost;

pub(crate) fn request<S: ShellHost>(state: &mut S, _bell: XdgSystemBellV1, request: Request) {
    let Request::Ring { surface } = request else {
        return;
    };
    let surface = surface.and_then(|surface| state.surfaces().lookup(&surface));
    state.shell_event(ShellEvent::Bell { surface });
}
