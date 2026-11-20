use music_core::{Id, Result};
use music_spotify_wire::SpotifyId;

use crate::failure::unreadable;

pub(crate) const TRACK: &str = "spotify:track:";
pub(crate) const ALBUM: &str = "spotify:album:";
pub(crate) const ARTIST: &str = "spotify:artist:";
pub(crate) const PLAYLIST: &str = "spotify:playlist:";

const IMAGES: &str = "https://i.scdn.co/image/";

pub(crate) fn compose(kind: &str, id: &Id) -> String {
    format!("{kind}{id}")
}

pub(crate) fn spotify_id(id: &Id) -> Result<SpotifyId> {
    SpotifyId::from_base62(id.as_str()).map_err(unreadable)
}

pub(crate) fn id_in(uri: &str, kind: &str) -> Option<Id> {
    uri.strip_prefix(kind)
        .filter(|id| !id.is_empty())
        .map(Id::new)
}

pub(crate) fn gid(raw: &[u8]) -> Option<Id> {
    SpotifyId::from_gid(raw)
        .ok()
        .map(|id| Id::new(id.to_base62()))
}

pub(crate) fn gid_uri(kind: &str, raw: &[u8]) -> Option<String> {
    gid(raw).map(|id| compose(kind, &id))
}

pub(crate) fn image(file: &[u8]) -> Option<String> {
    if file.is_empty() {
        return None;
    }
    let hex = rikka_types::hex::encode(file);
    Some(format!("{IMAGES}{hex}"))
}
