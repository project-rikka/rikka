use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

use rikka_image::{ColorScheme, ImageLoader};
use rikka_worker::Observable;

const NAME: &str = "rikka-footprints";

const MEASURED: u32 = 96;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Drawing {
    Raster,
    Vector,
}

#[derive(Debug)]
pub(crate) struct Ask {
    pub(crate) path: Arc<Path>,
    pub(crate) drawing: Drawing,
    pub(crate) scheme: ColorScheme,
}

#[derive(Debug)]
pub(super) struct Measurement {
    pub(super) path: Arc<Path>,
    pub(super) scale: f32,
    pub(super) scheme: ColorScheme,
    pub(super) follows_scheme: bool,
}

pub(crate) fn start(
    asks: Receiver<Ask>,
    answers: Sender<Measurement>,
    measured: Observable<()>,
) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name(NAME.to_owned())
        .spawn(move || {
            let loader = ImageLoader::with_threads(1);
            for ask in asks {
                if answers.send(measure(&loader, ask)).is_err() {
                    return;
                }
                measured.set(());
            }
        })?;
    Ok(())
}

fn measure(loader: &ImageLoader, ask: Ask) -> Measurement {
    let request = loader.read(&*ask.path).color_scheme(ask.scheme);
    let request = match ask.drawing {
        Drawing::Raster => request.thumbnail(MEASURED, MEASURED),
        Drawing::Vector => request.vector(MEASURED, MEASURED),
    };
    let handle = request.submit();
    let scale = handle
        .wait()
        .inspect_err(|error| {
            tracing::debug!("failed to measure icon {}: {error}", ask.path.display());
        })
        .ok()
        .and_then(|image| {
            let (width, height) = image.dimensions();
            icon_footprint::scale(width, height, &image.rgba8())
        });
    Measurement {
        path: ask.path,
        scale: scale.unwrap_or(1.0),
        scheme: ask.scheme,
        follows_scheme: handle.follows_color_scheme(),
    }
}
