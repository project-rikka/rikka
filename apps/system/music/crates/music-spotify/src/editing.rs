use music_core::{Error, Id, Playlist, Result};
use music_spotify_api::SpClient;

use crate::failure;
use crate::playlists;
use crate::uri::{self, PLAYLIST, TRACK};

pub(crate) async fn create(api: &SpClient, name: &str, description: &str) -> Result<Playlist> {
    let created = api
        .create_playlist(name, description)
        .await
        .map_err(failure::api)?;
    let id = uri::id_in(&created, PLAYLIST)
        .ok_or_else(|| Error::Unreadable(format!("a new playlist named {created}")))?;
    Ok(Playlist {
        id,
        name: name.to_owned(),
        owner: api.username().to_owned(),
        description: description.to_owned(),
        artwork: Default::default(),
        length: 0,
        editable: true,
    })
}

pub(crate) async fn rename(api: &SpClient, playlist: &Id, name: &str) -> Result<()> {
    api.rename_playlist(uri::spotify_id(playlist)?, name)
        .await
        .map_err(failure::api)
}

pub(crate) async fn forget(api: &SpClient, playlist: &Id) -> Result<()> {
    api.unfollow_playlist(&uri::compose(PLAYLIST, playlist))
        .await
        .map_err(failure::api)
}

pub(crate) async fn add(api: &SpClient, playlist: &Id, tracks: &[Id]) -> Result<()> {
    api.append_items(uri::spotify_id(playlist)?, &track_uris(tracks))
        .await
        .map_err(failure::api)
}

pub(crate) async fn remove(api: &SpClient, playlist: &Id, track: &Id) -> Result<()> {
    api.remove_items(uri::spotify_id(playlist)?, &[uri::compose(TRACK, track)])
        .await
        .map_err(failure::api)
}

pub(crate) async fn arrange(api: &SpClient, playlist: &Id, tracks: &[Id]) -> Result<()> {
    let mut current = playlists::item_uris(api, playlist).await?;
    current.sort_unstable();
    current.dedup();
    api.replace_items(uri::spotify_id(playlist)?, &current, &track_uris(tracks))
        .await
        .map_err(failure::api)
}

fn track_uris(tracks: &[Id]) -> Vec<String> {
    tracks
        .iter()
        .map(|track| uri::compose(TRACK, track))
        .collect()
}
