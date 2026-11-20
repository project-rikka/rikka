use rikka_wayland_client::wl_pointer::WlPointer;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::zwp_locked_pointer_v1::ZwpLockedPointerV1;
use rikka_wayland_client::zwp_pointer_constraints_v1::{Lifetime, ZwpPointerConstraintsV1};

use crate::tasks::WindowId;

#[derive(Debug)]
pub(crate) struct Locked(ZwpLockedPointerV1);

impl Locked {
    pub(crate) fn hold(
        constraints: &ZwpPointerConstraintsV1,
        surface: &WlSurface,
        pointer: &WlPointer,
        id: WindowId,
    ) -> Locked {
        Locked(
            constraints
                .lock_pointer(surface, pointer, None, Lifetime::Persistent)
                .with_key(id),
        )
    }
}

impl Drop for Locked {
    fn drop(&mut self) {
        self.0.destroy();
    }
}
