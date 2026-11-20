use std::sync::Arc;

use rikka_mpris::{Art, Track};
use rikka_theme::Corner;
use rikka_ui::{Element, ImageSource, Length, MaterialSymbol, Styled, View, fetch, palette};
use rikka_widgets::{ImageFit, container, icon, image};

pub const BLANK_SYMBOL: f32 = 0.55;

const FETCHED: &str = "island.art";

#[derive(Clone, Debug)]
pub enum Cover {
    Picture(ImageSource),
    Blank,
}

#[must_use]
pub fn cover(track: Option<&Track>) -> Cover {
    match track.and_then(|track| track.art.clone()) {
        Some(Art::File(path)) => Cover::Picture(ImageSource::file(path)),
        Some(Art::Web(url)) => fetch((FETCHED, url.clone()), move || download(url))
            .ready()
            .map_or(Cover::Blank, |bytes: &Arc<[u8]>| {
                Cover::Picture(ImageSource::Bytes(Arc::clone(bytes)))
            }),
        None => Cover::Blank,
    }
}

pub fn art(track: Option<&Track>, side: f32, corner: Corner) -> Element {
    match cover(track) {
        Cover::Picture(source) => image(source)
            .fit(ImageFit::Cover)
            .radius(corner)
            .width(Length::px(side))
            .height(Length::px(side))
            .build(),
        Cover::Blank => container(
            icon(MaterialSymbol::MUSIC_NOTE)
                .extent(Length::px(side * BLANK_SYMBOL))
                .color(palette().text_muted),
        )
        .center()
        .background(palette().surface_variant)
        .radius(corner)
        .width(Length::px(side))
        .height(Length::px(side))
        .build(),
    }
}

async fn download(url: String) -> rikka_net::Result<Arc<[u8]>> {
    Ok(Arc::from(&rikka_net::get_bytes(url.as_str()).await?[..]))
}
