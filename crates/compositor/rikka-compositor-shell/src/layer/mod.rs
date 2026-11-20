mod commands;
mod requests;
mod role;
mod surface;

use rikka_compositor_protocols::zwlr_layer_shell_v1::ZwlrLayerShellV1;
use rikka_compositor_protocols::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;
use rikka_compositor_server::{DisplayHandle, Module};

pub use surface::{LayerState, LayerSurface, Margins};

pub(crate) use role::LayerRole;

use crate::host::ShellHost;

#[derive(Debug, Clone, Copy, Default)]
pub struct LayerShell;

impl<S: ShellHost> Module<S> for LayerShell {
    fn install(self, display: &DisplayHandle<S>) {
        display.serve::<ZwlrLayerShellV1>(requests::shell);
        display
            .serve::<ZwlrLayerSurfaceV1>(requests::request)
            .destroyed(requests::destroyed);
        display.global::<ZwlrLayerShellV1>().privileged().publish();
    }
}
