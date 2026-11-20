mod requests;
mod session;
mod surface;

use rikka_compositor_protocols::ext_session_lock_manager_v1::ExtSessionLockManagerV1;
use rikka_compositor_protocols::ext_session_lock_surface_v1::ExtSessionLockSurfaceV1;
use rikka_compositor_protocols::ext_session_lock_v1::ExtSessionLockV1;
use rikka_compositor_server::{DisplayHandle, Module};

pub use surface::LockSurface;

pub(crate) use session::Locks;
pub(crate) use surface::LockRole;

use crate::host::ShellHost;

#[derive(Debug, Clone, Copy, Default)]
pub struct SessionLock;

impl<S: ShellHost> Module<S> for SessionLock {
    fn install(self, display: &DisplayHandle<S>) {
        display.serve::<ExtSessionLockManagerV1>(requests::manager);
        display
            .serve::<ExtSessionLockV1>(requests::request)
            .destroyed(requests::destroyed);
        display
            .serve::<ExtSessionLockSurfaceV1>(surface::request)
            .destroyed(surface::destroyed);
        display
            .global::<ExtSessionLockManagerV1>()
            .privileged()
            .publish();
    }
}
