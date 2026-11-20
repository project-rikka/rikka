use std::collections::HashMap;
use std::time::Duration;

use music_core::{
    Adapter, Album, AlbumRef, Artist, Artwork, Credit, Id, Isrc, Picture, Release, Result, Track,
};
use music_spotify_api::{
    Album as AlbumMessage, AlbumType, Artist as ArtistMessage, Entity, Image, ImageSize, SpClient,
    Track as TrackMessage,
};

use crate::failure;
use crate::provider::Spotify;
use crate::uri::{self, ALBUM, ARTIST, TRACK};

const ISRC: &str = "isrc";

pub(crate) async fn tracks(api: &SpClient, uris: &[String]) -> Result<Vec<Track>> {
    let known = entities::<TrackMessage>(api, uris).await?;
    Ok(in_order(uris, known, track))
}

pub(crate) async fn albums(api: &SpClient, uris: &[String]) -> Result<Vec<Album>> {
    let known = entities::<AlbumMessage>(api, uris).await?;
    Ok(in_order(uris, known, album))
}

pub(crate) async fn artists(api: &SpClient, uris: &[String]) -> Result<Vec<Artist>> {
    let known = entities::<ArtistMessage>(api, uris).await?;
    Ok(in_order(uris, known, artist))
}

pub(crate) async fn message<M: Entity>(api: &SpClient, uri: &str) -> Result<Option<M>> {
    let uris = [uri.to_owned()];
    Ok(entities::<M>(api, &uris).await?.remove(uri))
}

async fn entities<M: Entity>(api: &SpClient, uris: &[String]) -> Result<HashMap<String, M>> {
    api.entities::<M>(uris).await.map_err(failure::api)
}

fn in_order<M, T>(
    uris: &[String],
    mut known: HashMap<String, M>,
    convert: impl Fn(&str, &M) -> Option<T>,
) -> Vec<T> {
    uris.iter()
        .filter_map(|uri| {
            let message = known.remove(uri)?;
            convert(uri, &message)
        })
        .collect()
}

pub(crate) fn track(uri: &str, message: &TrackMessage) -> Option<Track> {
    let album = message.album.as_ref();
    Some(Track {
        service: Spotify::SERVICE,
        id: uri::id_in(uri, TRACK)?,
        name: message.name.clone(),
        artists: credits(&message.artist),
        album: AlbumRef {
            id: album.and_then(|album| uri::gid(&album.gid)),
            name: album.map(|album| album.name.clone()).unwrap_or_default(),
        },
        artwork: album.map(album_artwork).unwrap_or_default(),
        duration: Duration::from_millis(u64::try_from(message.duration).unwrap_or(0)),
        explicit: message.explicit,
        playable: !message.file.is_empty() || !message.alternative.is_empty(),
        number: u32::try_from(message.number).unwrap_or(0),
        fidelity: None,
        isrc: message
            .external_id
            .iter()
            .find(|external| external.kind.eq_ignore_ascii_case(ISRC))
            .and_then(|external| Isrc::parse(&external.id)),
    })
}

pub(crate) fn album(uri: &str, message: &AlbumMessage) -> Option<Album> {
    Some(Album {
        id: uri::id_in(uri, ALBUM)?,
        name: message.name.clone(),
        artists: credits(&message.artist),
        artwork: album_artwork(message),
        release: match message.album_type() {
            AlbumType::Single => Release::Single,
            AlbumType::Ep => Release::Ep,
            AlbumType::Compilation => Release::Compilation,
            AlbumType::Album | AlbumType::Audiobook | AlbumType::Podcast => Release::Album,
        },
        year: message
            .date
            .as_ref()
            .map(|date| date.year)
            .filter(|year| *year > 0),
        fidelity: None,
    })
}

pub(crate) fn artist(uri: &str, message: &ArtistMessage) -> Option<Artist> {
    let mut images = message
        .portrait_group
        .as_ref()
        .map(|group| group.image.clone())
        .unwrap_or_default();
    images.extend(message.portrait.iter().cloned());
    Some(Artist {
        id: uri::id_in(uri, ARTIST)?,
        name: message.name.clone(),
        artwork: artwork(&images),
    })
}

pub(crate) fn album_track_uris(message: &AlbumMessage) -> Vec<String> {
    message
        .disc
        .iter()
        .flat_map(|disc| disc.track.iter())
        .filter_map(|track| uri::gid_uri(TRACK, &track.gid))
        .collect()
}

fn credits(artists: &[ArtistMessage]) -> Vec<Credit> {
    artists
        .iter()
        .filter(|artist| !artist.name.is_empty())
        .map(|artist| Credit {
            id: uri::gid(&artist.gid),
            name: artist.name.clone(),
        })
        .collect()
}

fn album_artwork(album: &AlbumMessage) -> Artwork {
    let mut images = album
        .cover_group
        .as_ref()
        .map(|group| group.image.clone())
        .unwrap_or_default();
    images.extend(album.cover.iter().cloned());
    artwork(&images)
}

fn artwork(images: &[Image]) -> Artwork {
    Artwork::new(
        images
            .iter()
            .filter_map(|image| {
                let url = uri::image(&image.file_id)?;
                let width = match u32::try_from(image.width) {
                    Ok(width) if width > 0 => width,
                    _ => nominal_width(image.size()),
                };
                Some(Picture { url, width })
            })
            .collect(),
    )
}

fn nominal_width(size: ImageSize) -> u32 {
    match size {
        ImageSize::Small => 64,
        ImageSize::Default => 300,
        ImageSize::Large => 640,
        ImageSize::Xlarge => 1280,
    }
}

pub(crate) fn uris_of(kind: &str, ids: impl IntoIterator<Item = Id>) -> Vec<String> {
    ids.into_iter().map(|id| uri::compose(kind, &id)).collect()
}
