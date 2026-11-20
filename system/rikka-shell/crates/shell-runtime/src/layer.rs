use rikka_wayland_client::Proxy;
use rikka_wayland_client::wl_compositor::WlCompositor;
use rikka_wayland_client::wl_output::WlOutput;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::zwlr_layer_shell_v1::{self, ZwlrLayerShellV1};
use rikka_wayland_client::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;

#[derive(Debug)]
pub(super) struct LayerSurface {
    role: ZwlrLayerSurfaceV1,
    surface: WlSurface,
}

impl LayerSurface {
    pub(crate) fn new(
        compositor: &WlCompositor,
        shell: &ZwlrLayerShellV1,
        output: Option<&WlOutput>,
        layer: zwlr_layer_shell_v1::Layer,
        namespace: String,
    ) -> LayerSurface {
        let surface = compositor.create_surface();
        let role = shell
            .get_layer_surface(&surface, output, layer, &namespace)
            .with_key(surface.object_id());
        LayerSurface { role, surface }
    }

    pub(crate) fn surface(&self) -> &WlSurface {
        &self.surface
    }

    pub(crate) fn role(&self) -> &ZwlrLayerSurfaceV1 {
        &self.role
    }

    pub(crate) fn commit(&self) {
        self.surface.commit();
    }
}

impl Drop for LayerSurface {
    fn drop(&mut self) {
        self.role.destroy();
        self.surface.destroy();
    }
}
