use rikka_compositor_desktop::Target;
use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_shell::shell_role;
use rikka_compositor_surface::{SurfaceId, Surfaces};

use crate::context::InputRef;
use crate::windows::Windows;

pub(crate) fn surface_origin(
    windows: &Windows,
    surfaces: &Surfaces,
    surface: SurfaceId,
) -> Option<Point<i32, Logical>> {
    let root = surfaces.root(surface);
    let desktop = &windows.desktop;
    let target = shell_role(surfaces, root)
        .and_then(|role| desktop.target_for_role(role))
        .or_else(|| windows.window_of(surfaces, root).map(Target::Window))?;
    let corner = match target {
        Target::Window(window) => desktop.window(window)?.surface_location(),
        Target::Layer(layer) => desktop.layer(layer)?.geometry().loc,
        Target::Popup(popup) => desktop.popup_location(popup)?,
        Target::Lock(lock) => {
            desktop
                .outputs()
                .find(|(output, _)| {
                    desktop
                        .lock_surface_on(*output)
                        .is_some_and(|(locked, _)| locked == lock)
                })?
                .1
                .geometry()
                .loc
        }
    };
    let within = surfaces
        .tree(root)
        .find(|placed| placed.surface == surface)?
        .location;
    Some(corner + within)
}

impl InputRef<'_> {
    pub(crate) fn surface_origin(&self, surface: SurfaceId) -> Option<Point<i32, Logical>> {
        surface_origin(self.windows, self.surfaces, surface)
    }
}
