use rikka_compositor_protocols::wl_output::WlOutput;
use rikka_compositor_protocols::zxdg_output_manager_v1::ZxdgOutputManagerV1;
use rikka_compositor_protocols::zxdg_output_v1::ZxdgOutputV1;
use rikka_compositor_server::{DisplayHandle, Module};

use super::host::OutputHost;
use super::protocol;

#[derive(Debug, Clone, Copy, Default)]
pub struct OutputProtocols;

impl<S: OutputHost> Module<S> for OutputProtocols {
    fn install(self, display: &DisplayHandle<S>) {
        display
            .serve::<WlOutput>(protocol::request)
            .bound(protocol::bound)
            .destroyed(protocol::destroyed);
        display.serve::<ZxdgOutputManagerV1>(protocol::manager);
        display
            .serve::<ZxdgOutputV1>(protocol::xdg_request)
            .destroyed(protocol::xdg_destroyed);
        display.global::<ZxdgOutputManagerV1>().publish();
    }
}
