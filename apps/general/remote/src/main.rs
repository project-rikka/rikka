#![cfg_attr(windows, windows_subsystem = "windows")]

mod computer;
mod live;
mod screen;
mod session;
mod tokens;
mod viewer;

use rikka_app::WindowShape;
use rikka_app::prelude::*;

use crate::viewer::Viewer;

rikka_app::mimalloc!();

fn main() -> rikka_app::Result<()> {
    App::new("rikka-remote-viewer")
        .i18n(rikka_i18n::embedded!())
        .decodes_video()
        .windows(|_: LaunchRequest| window())
        .run()
}

fn window() -> Window<Viewer> {
    Window::new(Viewer::new, viewer::view)
        .title(tr!("remote.title"))
        .size(1280.0, 800.0)
        .min_size(480.0, 360.0)
        .on_shape(|viewer: &mut Viewer, shape: WindowShape| viewer.set_fullscreen(shape.fullscreen))
}
