use rikka_compositor_surface::{SurfaceId, Surfaces};

use crate::ids::{LayerId, LockSurfaceId, PopupId, ToplevelId};
use crate::layer::LayerRole;
use crate::lock::LockRole;
use crate::xdg::{XdgObject, XdgRole};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShellRole {
    Toplevel(ToplevelId),
    Popup(PopupId),
    Layer(LayerId),
    LockSurface(LockSurfaceId),
}

pub fn shell_role(surfaces: &Surfaces, surface: SurfaceId) -> Option<ShellRole> {
    if let Some(xdg) = surfaces.role::<XdgRole>(surface) {
        return xdg.object.map(|object| match object {
            XdgObject::Toplevel(toplevel) => ShellRole::Toplevel(toplevel),
            XdgObject::Popup(popup) => ShellRole::Popup(popup),
        });
    }
    if let Some(layer) = surfaces.role::<LayerRole>(surface) {
        return Some(ShellRole::Layer(layer.id));
    }
    surfaces
        .role::<LockRole>(surface)
        .map(|lock| ShellRole::LockSurface(lock.id))
}
