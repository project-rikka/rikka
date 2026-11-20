use std::sync::Arc;

use futures_util::future::join4;
use music_core::{
    Album, AlbumPage, Artist, ArtistPage, Card, Catalog, Error, Found, Home, Id, Isrc, Playlist,
    PlaylistPage, Release, Result, Shared, Shelf, Track, async_trait, lookup_by_isrc,
};
use music_stream::OffsetPaging;
use serde_json::Value;

use crate::api::{Api, Signed};
use crate::parse::{self, items};

const PAGE: usize = 500;
const SEARCHED: &str = "30";
const LINKS: &str = "https://open.qobuz.com";
const EDITED: usize = 100;

pub(crate) struct QobuzCatalog {
    api: Arc<Api>,
    me: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Duplicates {
    Skip,
    Keep,
}

impl QobuzCatalog {
    pub(crate) fn new(api: Arc<Api>, me: u64) -> QobuzCatalog {
        QobuzCatalog { api, me }
    }

    async fn listing(
        &self,
        method: &str,
        key: &str,
        arguments: &[(&str, String)],
        signed: Signed,
    ) -> Result<(Value, Vec<Value>)> {
        let mut paging = OffsetPaging::new(PAGE);
        let mut first = None;
        let mut listed = Vec::new();
        while let Some(offset) = paging.next_offset() {
            let mut query = arguments.to_vec();
            query.push(("limit", paging.size().to_string()));
            query.push(("offset", offset.to_string()));
            let answer = self.api.call(method, &query, signed).await?;
            let page = items(&answer, key);
            let total = answer
                .get(key)
                .and_then(|list| list.get("total"))
                .and_then(Value::as_u64)
                .and_then(|total| usize::try_from(total).ok());
            paging.arrived(page.len(), total);
            listed.extend(page.iter().cloned());
            first.get_or_insert(answer);
        }
        Ok((first.unwrap_or_default(), listed))
    }

    async fn favorites(&self, kind: &str) -> Result<Vec<Value>> {
        let arguments = [("type", kind.to_owned())];
        self.listing("favorite/getUserFavorites", kind, &arguments, Signed::Bare)
            .await
            .map(|(_, favorites)| favorites)
    }

    async fn own_playlists(&self) -> Result<Vec<Playlist>> {
        let (_, playlists) = self
            .listing(
                "playlist/getUserPlaylists",
                "playlists",
                &[],
                Signed::WithArguments,
            )
            .await?;
        Ok(playlists
            .iter()
            .filter_map(|value| parse::playlist(value, self.me))
            .collect())
    }

    async fn delete_slots(&self, playlist: &Id, slots: &[String]) -> Result<()> {
        self.api
            .get(
                "playlist/deleteTracks",
                &[
                    ("playlist_id", playlist.to_string()),
                    ("playlist_track_ids", slots.join(",")),
                ],
            )
            .await
            .map(drop)
    }

    async fn add_tracks(&self, playlist: &Id, tracks: &[Id], duplicates: Duplicates) -> Result<()> {
        let skip = duplicates == Duplicates::Skip;
        for batch in tracks.chunks(EDITED) {
            let listed = batch.iter().map(Id::as_str).collect::<Vec<_>>().join(",");
            self.api
                .get(
                    "playlist/addTracks",
                    &[
                        ("playlist_id", playlist.to_string()),
                        ("track_ids", listed),
                        ("no_duplicate", skip.to_string()),
                    ],
                )
                .await?;
        }
        Ok(())
    }

    async fn playlist_tracks(&self, id: &Id) -> Result<(Value, Vec<Value>)> {
        let arguments = [
            ("playlist_id", id.to_string()),
            ("extra", "tracks".to_owned()),
        ];
        self.listing("playlist/get", "tracks", &arguments, Signed::WithArguments)
            .await
    }

    async fn discovered(&self) -> Result<Vec<Shelf>> {
        let answer = self.api.get("discover/index", &[]).await?;
        let containers = answer.get("containers").unwrap_or(&Value::Null);
        let data = |key: &str| {
            containers
                .get(key)
                .map(|container| items(container, "data"))
                .unwrap_or(&[])
        };
        let albums = |key: &str| -> Arc<[Card]> {
            data(key)
                .iter()
                .filter_map(parse::album)
                .map(Card::Album)
                .collect()
        };
        let mut acclaimed: Vec<Card> = albums("qobuzissims").to_vec();
        acclaimed.extend(albums("press_awards").iter().cloned());
        Ok(vec![
            Shelf::NewReleases(albums("new_releases")),
            Shelf::Curated(
                data("playlists")
                    .iter()
                    .filter_map(|value| parse::playlist(value, self.me))
                    .map(Card::Playlist)
                    .collect(),
            ),
            Shelf::Acclaimed(acclaimed.into()),
            Shelf::MostPlayed(albums("most_streamed")),
        ])
    }
}

#[async_trait]
impl Catalog for QobuzCatalog {
    async fn home(&self) -> Result<Home> {
        let (discovered, playlists, albums, saved) = join4(
            self.discovered(),
            self.own_playlists(),
            self.favorites("albums"),
            self.saved_tracks(),
        )
        .await;
        Home::from_outcomes([
            saved.map(|tracks| vec![Shelf::QuickPicks(tracks.iter().take(20).cloned().collect())]),
            discovered,
            albums.map(|albums| {
                vec![Shelf::RecentAlbums(
                    albums
                        .iter()
                        .filter_map(parse::album)
                        .map(Card::Album)
                        .collect(),
                )]
            }),
            playlists.map(|playlists| {
                vec![Shelf::YourPlaylists(
                    playlists.into_iter().map(Card::Playlist).collect(),
                )]
            }),
        ])
    }

    async fn search(&self, query: &str) -> Result<Found> {
        let answer = self
            .api
            .get(
                "catalog/search",
                &[
                    ("query", query.to_owned()),
                    ("limit", SEARCHED.to_owned()),
                    ("offset", "0".to_owned()),
                ],
            )
            .await?;
        Ok(Found {
            tracks: items(&answer, "tracks")
                .iter()
                .filter_map(|value| parse::track(value, None))
                .collect(),
            albums: items(&answer, "albums")
                .iter()
                .filter_map(parse::album)
                .collect(),
            artists: items(&answer, "artists")
                .iter()
                .filter_map(parse::artist)
                .collect(),
            playlists: items(&answer, "playlists")
                .iter()
                .filter_map(|value| parse::playlist(value, self.me))
                .collect(),
        })
    }

    async fn saved_tracks(&self) -> Result<Arc<[Track]>> {
        Ok(self
            .favorites("tracks")
            .await?
            .iter()
            .filter_map(|value| parse::track(value, None))
            .collect())
    }

    async fn playlists(&self) -> Result<Arc<[Playlist]>> {
        Ok(self.own_playlists().await?.into())
    }

    async fn playlist(&self, id: &Id) -> Result<PlaylistPage> {
        let (answer, tracks) = self.playlist_tracks(id).await?;
        Ok(PlaylistPage {
            playlist: parse::playlist(&answer, self.me)
                .ok_or_else(|| Error::Unreadable(format!("playlist {id}")))?,
            tracks: tracks
                .iter()
                .filter_map(|value| parse::track(value, None))
                .collect(),
        })
    }

    async fn album(&self, id: &Id) -> Result<AlbumPage> {
        let answer = self
            .api
            .get("album/get", &[("album_id", id.to_string())])
            .await?;
        let album =
            parse::album(&answer).ok_or_else(|| Error::Unreadable(format!("album {id}")))?;
        let tracks = items(&answer, "tracks")
            .iter()
            .filter_map(|value| parse::track(value, Some(&album)))
            .collect();
        Ok(AlbumPage { album, tracks })
    }

    async fn artist(&self, id: &Id) -> Result<ArtistPage> {
        let answer = self
            .api
            .get("artist/page", &[("artist_id", id.to_string())])
            .await?;
        let artist: Artist =
            parse::artist(&answer).ok_or_else(|| Error::Unreadable(format!("artist {id}")))?;
        let mut albums: Vec<Album> = Vec::new();
        let mut singles: Vec<Album> = Vec::new();
        for group in answer
            .get("releases")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice)
        {
            let single = group.get("type").and_then(Value::as_str) == Some("epSingle");
            for value in items(group, "items") {
                let Some(album) = parse::album(value) else {
                    continue;
                };
                if albums
                    .iter()
                    .chain(&singles)
                    .any(|existing| existing.id == album.id)
                {
                    continue;
                }
                match single || matches!(album.release, Release::Single | Release::Ep) {
                    true => singles.push(album),
                    false => albums.push(album),
                }
            }
        }
        Ok(ArtistPage {
            popular: items(&answer, "top_tracks")
                .iter()
                .filter_map(|value| parse::track(value, None))
                .collect(),
            artist,
            albums: albums.into(),
            singles: singles.into(),
        })
    }

    async fn mix(&self, seed: &Track) -> Result<Arc<[Track]>> {
        let answer = self
            .api
            .get("radio/track", &[("track_id", seed.id.to_string())])
            .await?;
        Ok(items(&answer, "tracks")
            .iter()
            .filter_map(|value| parse::track(value, None))
            .collect())
    }

    async fn set_saved(&self, track: &Id, saved: bool) -> Result<()> {
        let method = match saved {
            true => "favorite/create",
            false => "favorite/delete",
        };
        self.api
            .get(method, &[("track_ids", track.to_string())])
            .await
            .map(drop)
    }

    async fn create_playlist(&self, name: &str, description: &str) -> Result<Playlist> {
        let answer = self
            .api
            .get(
                "playlist/create",
                &[
                    ("name", name.to_owned()),
                    ("description", description.to_owned()),
                    ("is_public", "false".to_owned()),
                    ("is_collaborative", "false".to_owned()),
                ],
            )
            .await?;
        parse::playlist(&answer, self.me)
            .ok_or_else(|| Error::Unreadable("the new playlist".into()))
    }

    async fn rename_playlist(&self, playlist: &Id, name: &str) -> Result<()> {
        self.api
            .get(
                "playlist/update",
                &[
                    ("playlist_id", playlist.to_string()),
                    ("name", name.to_owned()),
                ],
            )
            .await
            .map(drop)
    }

    async fn delete_playlist(&self, playlist: &Playlist) -> Result<()> {
        let method = match playlist.editable {
            true => "playlist/delete",
            false => "playlist/unsubscribe",
        };
        self.api
            .get(method, &[("playlist_id", playlist.id.to_string())])
            .await
            .map(drop)
    }

    async fn add_to_playlist(&self, playlist: &Id, tracks: &[Id]) -> Result<()> {
        self.add_tracks(playlist, tracks, Duplicates::Skip).await
    }

    async fn remove_from_playlist(&self, playlist: &Id, track: &Id) -> Result<()> {
        let (_, tracks) = self.playlist_tracks(playlist).await?;
        let slots = slots_of(&tracks, track);
        if slots.is_empty() {
            return Err(Error::Missing(format!("{track} in playlist {playlist}")));
        }
        self.delete_slots(playlist, &slots).await
    }

    async fn arrange_playlist(&self, playlist: &Id, tracks: &[Id]) -> Result<()> {
        let (_, current) = self.playlist_tracks(playlist).await?;
        let slots: Vec<String> = current
            .iter()
            .filter_map(|value| value.get("playlist_track_id").and_then(parse::id))
            .map(|slot| slot.to_string())
            .collect();
        self.add_tracks(playlist, tracks, Duplicates::Keep).await?;
        for batch in slots.chunks(EDITED) {
            self.delete_slots(playlist, batch).await?;
        }
        Ok(())
    }

    async fn lookup(&self, track: &Track) -> Result<Arc<[Track]>> {
        lookup_by_isrc(track, Isrc::to_string, |query| async move {
            self.search(&query).await
        })
        .await
    }

    fn link(&self, shared: Shared<'_>) -> String {
        match shared {
            Shared::Track(id) => format!("{LINKS}/track/{id}"),
            Shared::Playlist(id) => format!("{LINKS}/playlist/{id}"),
        }
    }
}

fn slots_of(tracks: &[Value], track: &Id) -> Vec<String> {
    tracks
        .iter()
        .filter(|value| value.get("id").and_then(parse::id).as_ref() == Some(track))
        .filter_map(|value| value.get("playlist_track_id").and_then(parse::id))
        .map(|slot| slot.to_string())
        .collect()
}
