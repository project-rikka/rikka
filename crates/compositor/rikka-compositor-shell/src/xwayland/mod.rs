mod role;

use rikka_compositor_protocols::xwayland_shell_v1::XwaylandShellV1;
use rikka_compositor_protocols::xwayland_surface_v1::XwaylandSurfaceV1;
use rikka_compositor_server::{Client, DisplayHandle, Module};

use crate::host::ShellHost;

#[derive(Debug, Clone, Copy, Default)]
pub struct XwaylandShell;

impl<S: ShellHost> Module<S> for XwaylandShell {
    fn install(self, display: &DisplayHandle<S>) {
        display.serve::<XwaylandShellV1>(role::shell);
        display.serve::<XwaylandSurfaceV1>(role::request);
        display
            .global::<XwaylandShellV1>()
            .only(Client::is_xwayland)
            .publish();
    }
}
