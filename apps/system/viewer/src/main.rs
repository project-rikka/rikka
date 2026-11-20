mod bar;
mod open_file;
mod screen;
mod transport;
mod viewer;
mod wording;

use rikka_app::prelude::*;
use rikka_ui_video::VideoStore;
use viewer_media::{Extent, Kind};

use crate::open_file::OpenFile;
use crate::viewer::Viewer;

const DEFAULT: (f32, f32) = (1000.0, 700.0);
const SMALLEST: (f32, f32) = (420.0, 300.0);
const LARGEST: (f32, f32) = (1600.0, 1000.0);
const CHROME: f32 = 52.0;

rikka_app::mimalloc!();

fn main() -> rikka_app::Result<()> {
    App::new("rikka-viewer")
        .draws_3d()
        .plays_video(VideoStore::default())
        .i18n(rikka_i18n::embedded!())
        .windows(windows)
        .run()
}

fn windows(open: LaunchRequest) -> Vec<Window<Viewer>> {
    let files: Vec<OpenFile> = open.paths().into_iter().map(OpenFile::open).collect();
    if files.is_empty() {
        return vec![window(Viewer::empty(), tr!("viewer.title"), DEFAULT)];
    }
    files
        .into_iter()
        .map(|file| {
            let title = Label::from(file.name());
            let size = size(&file);
            window(Viewer::open(file), title, size)
        })
        .collect()
}

fn window(viewer: Viewer, title: Label, size: (f32, f32)) -> Window<Viewer> {
    Window::new(move || viewer, viewer::view)
        .title(title)
        .size(size.0, size.1)
        .min_size(SMALLEST.0, SMALLEST.1)
        .on_start(|viewer: &mut Viewer| viewer.start())
}

fn size(file: &OpenFile) -> (f32, f32) {
    let Some(media) = file.media() else {
        return DEFAULT;
    };
    match (media.kind(), media.extent) {
        (Kind::Picture, Some(extent)) => window_size(extent),
        (Kind::Sound, _) => (560.0, 320.0),
        _ => DEFAULT,
    }
}

fn window_size(extent: Extent) -> (f32, f32) {
    let width = (extent.width as f32).clamp(SMALLEST.0, LARGEST.0);
    let height = (extent.height as f32 + CHROME).clamp(SMALLEST.1, LARGEST.1);
    (width, height)
}
