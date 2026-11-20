use std::time::Duration;

use rikka_compositor_desktop::Target;
use rikka_compositor_surface::{SurfaceId, Surfaces};

use super::Windows;
use crate::render::HIDDEN_FRAME_INTERVAL;

pub(crate) fn hidden_frame_pace(
    windows: &Windows,
    surfaces: &Surfaces,
    root: SurfaceId,
) -> Option<Duration> {
    let desktop = &windows.desktop;
    match desktop.root_of(windows.target_of(surfaces, root)?) {
        Target::Window(window) => desktop
            .window(window)?
            .visibility()
            .shown()
            .then_some(HIDDEN_FRAME_INTERVAL),
        Target::Layer(layer) => desktop
            .layer(layer)
            .is_some_and(|layer| layer.mapped())
            .then_some(HIDDEN_FRAME_INTERVAL),
        Target::Lock(_) => Some(HIDDEN_FRAME_INTERVAL),
        Target::Popup(_) => None,
    }
}
