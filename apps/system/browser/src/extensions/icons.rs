use std::collections::BTreeMap;

use rikka_app::prelude::*;
use rikka_image::ColorScheme;
use rikka_web::Runtime;

use crate::favicons;
use crate::shell::Shell;
use crate::widgets::favicon;
use browser_model::Favicon;

const ICON: &str = "rikka-browser-extension-icon";
const PACKAGED: &str = "moz-extension:";

#[derive(Clone, Debug, PartialEq)]
struct ExtensionIcon {
    url: String,
    side: f32,
}

fn closest(icons: &BTreeMap<String, String>, side: f32) -> Option<String> {
    let target = (side * 2.0).round() as i64;
    icons
        .iter()
        .min_by_key(|(size, _)| {
            let size: i64 = size.parse().unwrap_or(0);
            match size >= target {
                true => size - target,
                false => (target - size) * 4,
            }
        })
        .map(|(_, url)| url.clone())
}

pub(crate) fn icon(icons: Option<&BTreeMap<String, String>>, side: f32) -> Element {
    match icons.and_then(|icons| closest(icons, side)) {
        Some(url) => component_with(|shell: &Shell| shell, ExtensionIcon { url, side }, drawn),
        None => favicon(None, MaterialSymbol::EXTENSION, side),
    }
}

fn drawn(shell: &Shell, icon: &ExtensionIcon) -> Element {
    let key = shell.model().profile_here(shell.id());
    let engine = shell.browser().running_engine(key);
    let url = icon.url.clone();
    let scheme = shell.color_scheme();
    let loaded =
        move || async move { Ok::<_, std::convert::Infallible>(loaded(engine, url, scheme).await) };
    match fetch((ICON, icon.url.as_str()), loaded) {
        Fetch::Ready(image) => {
            let image: &Option<Favicon> = &image;
            favicon(image.as_ref(), MaterialSymbol::EXTENSION, icon.side)
        }
        _ => favicon(None, MaterialSymbol::EXTENSION, icon.side),
    }
}

async fn loaded(engine: Option<Runtime>, url: String, scheme: ColorScheme) -> Option<Favicon> {
    match (url.starts_with(PACKAGED), engine) {
        (true, Some(engine)) => {
            let icon = engine
                .extensions()
                .icon(url)
                .await
                .inspect_err(|error| tracing::debug!(%error, "failed to read extension icon"))
                .ok()?;
            favicons::decoded(&icon.bytes, scheme)
        }
        (true, None) => None,
        (false, _) => favicons::fetched(url, scheme).await,
    }
}
