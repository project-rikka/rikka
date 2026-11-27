use std::sync::Arc;

use google_account::Profile;
use rikka_app::prelude::*;

const KEY: &str = "google-avatar";

pub(crate) fn avatar(profile: &Profile, extent: f32) -> Element {
    let picture = profile.picture.clone().map(pictured);
    match picture {
        Some(Fetch::Ready(picture)) => image(ImageSource::Bytes(Arc::clone(&picture)))
            .fit(ImageFit::Cover)
            .radius(extent / 2.0)
            .size(extent, extent)
            .any(),
        _ => initial(&profile.email, extent),
    }
}

pub(crate) fn initial(name: &str, extent: f32) -> Element {
    let letter: String = name
        .chars()
        .next()
        .map(|letter| letter.to_uppercase().collect())
        .unwrap_or_default();
    container(
        text(letter)
            .text_size(extent * 0.46)
            .semibold()
            .color(palette().on_accent),
    )
    .center()
    .size(extent, extent)
    .radius(extent / 2.0)
    .background(palette().accent)
    .any()
}

fn pictured(url: String) -> Fetch<Arc<[u8]>> {
    fetch((KEY, url.clone()), move || async move {
        rikka_net::get_bytes(url)
            .await
            .map(|picture| Arc::<[u8]>::from(&picture[..]))
    })
}
