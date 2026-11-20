use rikka_app::Instance;
use rikka_i18n::I18n;
use rikka_ui_video::VideoStore;

use crate::abi::violation;
use crate::error;
use crate::ffi;
use crate::window::WindowBuilder;

#[derive(Debug)]
pub(crate) struct App {
    id: String,
    single: bool,
    windows: Vec<WindowBuilder>,
}

pub(crate) fn app_new(id: &str) -> App {
    let valid = !id.is_empty()
        && id.split('.').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || "_-".contains(character))
        });
    if !valid {
        violation(format!("`{id}` is not a reverse-DNS application id"));
    }
    App {
        id: id.to_owned(),
        single: false,
        windows: Vec::new(),
    }
}

pub(crate) fn app_set_single_instance(app: &mut App, single: bool) {
    app.single = single;
}

pub(crate) fn app_add_window(app: &mut App, window: WindowBuilder) {
    app.windows.push(window);
}

pub(crate) fn app_run(app: App) -> ffi::Status {
    let App {
        id,
        single,
        windows,
    } = app;
    let mut running = rikka_app::App::new(id)
        .i18n(I18n::builder())
        .plays_video(VideoStore::default());
    if single {
        running = running.instance(Instance::single());
    }
    for window in windows {
        running = running.window(window.into_window());
    }
    match running.run() {
        Ok(()) => ffi::Status::Ok,
        Err(failure) => error::fail(error::app_status(&failure), failure),
    }
}
