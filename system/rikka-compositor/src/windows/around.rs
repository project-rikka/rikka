use rikka_compositor_desktop::{Drawn, SceneLayers};
use rikka_compositor_geometry::{Logical, Rect};
use rikka_compositor_surface::{SurfaceId, Surfaces};
use rikka_compositor_types::OutputId;

use super::Windows;

pub(crate) fn drawn_on(
    windows: &Windows,
    surfaces: &Surfaces,
    output: OutputId,
    drawn: &mut Vec<Drawn>,
) {
    windows
        .desktop
        .drawn_into(surfaces, output, around(windows, surfaces, output), drawn);
}

pub(crate) fn around(windows: &Windows, surfaces: &Surfaces, output: OutputId) -> SceneLayers {
    let desktop = &windows.desktop;
    let Some(area) = desktop.output(output).map(|output| output.geometry()) else {
        return SceneLayers::ALL;
    };
    if overviewed(windows, output) {
        return SceneLayers::WITHOUT_WINDOWS;
    }
    let Some(window) = desktop
        .fullscreen_on(output)
        .and_then(|id| desktop.window(id))
    else {
        return SceneLayers::ALL;
    };
    let hides = window.surface().is_some_and(|surface| {
        surfaces.get(surface).is_some_and(|state| {
            let corner = window.surface_location();
            state.opaque().iter().any(|region| {
                Rect::<i32, Logical>::new(region.loc + corner, region.size).contains_rect(area)
            })
        })
    });
    SceneLayers {
        top: false,
        windows: true,
        backdrop: !hides,
    }
}

pub(crate) fn fullscreen_surface(windows: &Windows, output: OutputId) -> Option<SurfaceId> {
    if overviewed(windows, output) {
        return None;
    }
    let desktop = &windows.desktop;
    desktop
        .fullscreen_on(output)
        .and_then(|id| desktop.window(id))
        .and_then(|window| window.surface())
}

pub(crate) fn overviewed(windows: &Windows, output: OutputId) -> bool {
    let desktop = &windows.desktop;
    let Some(area) = desktop.output(output).map(|output| output.geometry()) else {
        return false;
    };
    windows.overviews.iter().any(|layer| {
        desktop.layer(*layer).is_some_and(|layer| {
            layer.output() == output
                && layer.mapped()
                && layer.geometry().size.w >= area.size.w
                && layer.geometry().size.h >= area.size.h
        })
    })
}
