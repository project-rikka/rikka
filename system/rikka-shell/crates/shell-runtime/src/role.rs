use rikka_wayland_client::ext_session_lock_surface_v1::ExtSessionLockSurfaceV1;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;

use super::layer::LayerSurface;

#[derive(Debug)]
pub(super) enum Role {
    Layer(LayerSurface),
    Lock(LockSurface),
}

#[derive(Debug)]
pub(super) struct LockSurface {
    role: ExtSessionLockSurfaceV1,
    surface: WlSurface,
}

impl LockSurface {
    pub(crate) fn new(role: ExtSessionLockSurfaceV1, surface: WlSurface) -> LockSurface {
        LockSurface { role, surface }
    }
}

impl Drop for LockSurface {
    fn drop(&mut self) {
        self.role.destroy();
        self.surface.destroy();
    }
}

impl Role {
    pub(crate) fn surface(&self) -> &WlSurface {
        match self {
            Role::Layer(layer) => layer.surface(),
            Role::Lock(lock) => &lock.surface,
        }
    }

    pub(crate) fn layer(&self) -> Option<&ZwlrLayerSurfaceV1> {
        match self {
            Role::Layer(layer) => Some(layer.role()),
            Role::Lock(_) => None,
        }
    }

    pub(super) fn ack_configure(&self, serial: u32) {
        match self {
            Role::Layer(layer) => layer.role().ack_configure(serial),
            Role::Lock(lock) => lock.role.ack_configure(serial),
        }
    }

    pub(crate) fn commit(&self) {
        self.surface().commit();
    }
}
