use music_core::Result;
use music_spotify_api::{Saved, SpClient};

use crate::failure;

pub(crate) const SAVED: &str = "collection";

pub(crate) async fn saved(api: &SpClient, set: &str, kind: &str) -> Result<Vec<Saved>> {
    let mut items: Vec<Saved> = api
        .saved(set)
        .await
        .map_err(failure::api)?
        .into_iter()
        .filter(|item| item.uri.starts_with(kind))
        .collect();
    items.sort_by_key(|item| std::cmp::Reverse(item.added));
    Ok(items)
}

pub(crate) async fn set_saved(api: &SpClient, set: &str, uri: &str, saved: bool) -> Result<()> {
    api.set_saved(set, uri, saved).await.map_err(failure::api)
}
