use std::sync::Arc;

use browser_model::Favicon;
use browser_store::Store;
use rikka_app::prelude::*;
use rikka_ui::ImageSource;

const STORED: &str = "rikka-browser-stored-favicon";

pub(crate) fn favicon(favicon: Option<&Favicon>, fallback: MaterialSymbol, side: f32) -> Element {
    match favicon {
        Some(Favicon { svg: Some(svg), .. }) => {
            sized(image(ImageSource::vector(Arc::clone(svg))), side)
        }
        Some(favicon) => picture(favicon.width, favicon.height, favicon.rgba.clone(), side),
        None => icon(fallback)
            .extent(Length::px(side))
            .color(palette().text_muted)
            .any(),
    }
}

pub(crate) fn stored_favicon(
    store: Option<&Store>,
    url: &str,
    fallback: MaterialSymbol,
    side: f32,
) -> Element {
    let Some(store) = store.cloned() else {
        return favicon(None, fallback, side);
    };
    let page = url.to_owned();
    match fetch_blocking((STORED, url), move || store.favicon_for_url(&page)) {
        Fetch::Ready(stored) => match stored.as_ref() {
            Some(stored) => picture(stored.width, stored.height, stored.rgba.clone(), side),
            _ => favicon(None, fallback, side),
        },
        _ => favicon(None, fallback, side),
    }
}

fn picture(width: u32, height: u32, pixels: Arc<[u8]>, side: f32) -> Element {
    let pixels = image(ImageSource::Rgba {
        width,
        height,
        pixels,
    });
    sized(pixels, side)
}

fn sized(picture: Image, side: f32) -> Element {
    picture.size(Length::px(side), Length::px(side)).any()
}
