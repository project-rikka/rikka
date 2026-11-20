use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use futures_util::future::{join5, try_join, try_join3, try_join4};
use futures_util::{StreamExt, TryStreamExt, stream};
use music_core::{
    Album, AlbumPage, Artist, ArtistPage, Card, Catalog, Error, Found, Home, Id, Playlist,
    PlaylistPage, Release, Result, Shared, Shelf, Track, async_trait,
};
use rikka_net::Method;
use serde_json::{Value, json};

use crate::api::Api;
use crate::parse::{self, collection};

const SEARCHED: usize = 20;
const LIKED: usize = 2000;
const SETS: usize = 500;
const SHELVED_SETS: usize = 20;
const PAGE: usize = 200;
const FOLLOWED: usize = 50;
const RECENT: usize = 25;
const STREAMED: usize = 50;
const POPULAR: usize = 20;
const RELEASES: usize = 100;
const RELATED: usize = 50;
const BATCH: usize = 50;
const BATCHES_AT_ONCE: usize = 4;
const LONGEST_SET: usize = 500;
const SYSTEM: &str = "soundcloud:system-playlists:";

pub(crate) struct SoundCloudCatalog {
    api: Arc<Api>,
    me: String,
}

impl SoundCloudCatalog {
    pub(crate) fn new(api: Arc<Api>, me: String) -> SoundCloudCatalog {
        SoundCloudCatalog { api, me }
    }

    async fn paginate(&self, path: &str, most: usize) -> Result<Vec<Value>> {
        self.api
            .paginate_all(path, &[("limit", most.min(PAGE).to_string())], most)
            .await
    }

    async fn set(&self, id: &Id) -> Result<Value> {
        let path = match id.as_str().starts_with(SYSTEM) {
            true => format!("system-playlists/{id}"),
            false => format!("playlists/{id}"),
        };
        self.api
            .get(&path, &[("representation", "full".to_owned())])
            .await
    }

    async fn listed(&self, mut set: Value) -> Result<Vec<(u32, Value)>> {
        let listed = match set.get_mut("tracks").map(Value::take) {
            Some(Value::Array(listed)) => listed,
            _ => Vec::new(),
        };
        let missing: Vec<Id> = listed
            .iter()
            .filter(|value| parse::text(value, "title").is_none())
            .filter_map(|value| value.get("id").and_then(parse::id))
            .collect();
        let batches: Vec<Vec<Value>> = stream::iter(missing.chunks(BATCH).map(<[Id]>::to_vec))
            .map(|batch| async move { self.batch(&batch).await })
            .buffered(BATCHES_AT_ONCE)
            .try_collect()
            .await?;
        let fetched: HashMap<Id, Value> = batches
            .into_iter()
            .flatten()
            .filter_map(|value| Some((value.get("id").and_then(parse::id)?, value)))
            .collect();
        Ok(listed
            .into_iter()
            .zip(1..)
            .filter_map(|(value, number)| match parse::text(&value, "title") {
                Some(_) => Some((number, value)),
                None => fetched
                    .get(&value.get("id").and_then(parse::id)?)
                    .map(|filled| (number, filled.clone())),
            })
            .collect())
    }

    async fn batch(&self, ids: &[Id]) -> Result<Vec<Value>> {
        let listed = ids.iter().map(Id::as_str).collect::<Vec<_>>().join(",");
        match self.api.get("tracks", &[("ids", listed)]).await? {
            Value::Array(tracks) => Ok(tracks),
            _ => Ok(Vec::new()),
        }
    }

    async fn sets_within(&self, most: usize) -> Result<Vec<Playlist>> {
        let (own, liked) = try_join(
            self.paginate(&format!("users/{}/playlists_without_albums", self.me), most),
            self.paginate(&format!("users/{}/playlist_likes", self.me), most),
        )
        .await?;
        let mut seen = HashSet::new();
        Ok(own
            .iter()
            .chain(liked.iter().filter_map(parse::liked_set))
            .filter_map(|value| parse::playlist(value, &self.me))
            .filter(|playlist| seen.insert(playlist.id.clone()))
            .collect())
    }

    async fn track_ids(&self, playlist: &Id) -> Result<Vec<u64>> {
        let set = self.set(playlist).await?;
        Ok(set
            .get("tracks")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .filter_map(|track| track.get("id").and_then(Value::as_u64))
            .collect())
    }

    async fn save_tracks(&self, playlist: &Id, tracks: &[u64]) -> Result<()> {
        if tracks.len() > LONGEST_SET {
            return Err(Error::Refused(format!(
                "a SoundCloud playlist holds at most {LONGEST_SET} tracks"
            )));
        }
        self.api
            .send(
                Method::PUT,
                &format!("playlists/{playlist}"),
                Some(&json!({ "playlist": { "tracks": tracks } })),
            )
            .await
            .map(drop)
    }

    async fn history(&self) -> Result<Vec<Track>> {
        let played = self.paginate("me/play-history/tracks", RECENT).await?;
        Ok(unique(
            played
                .iter()
                .filter_map(|item| item.get("track"))
                .filter_map(parse::track),
        ))
    }

    async fn streamed(&self) -> Result<Vec<Track>> {
        let answer = self
            .api
            .get("stream", &[("limit", STREAMED.to_string())])
            .await?;
        Ok(unique(
            collection(&answer)
                .iter()
                .filter_map(|item| item.get("track"))
                .filter_map(parse::track),
        ))
    }

    async fn selections(&self) -> Result<Vec<Shelf>> {
        let answer = self.api.get("mixed-selections", &[]).await?;
        Ok(parse::selections(&answer, &self.me))
    }

    async fn followed(&self) -> Result<Vec<Artist>> {
        let followed = self
            .paginate(&format!("users/{}/followings", self.me), FOLLOWED)
            .await?;
        Ok(followed.iter().filter_map(parse::artist).collect())
    }

    async fn searched(&self, category: &str, query: &str) -> Result<Vec<Value>> {
        let answer = self
            .api
            .get(
                &format!("search/{category}"),
                &[("q", query.to_owned()), ("limit", SEARCHED.to_string())],
            )
            .await?;
        Ok(collection(&answer).to_vec())
    }
}

#[async_trait]
impl Catalog for SoundCloudCatalog {
    async fn home(&self) -> Result<Home> {
        let (history, streamed, selections, playlists, followed) = join5(
            self.history(),
            self.streamed(),
            self.selections(),
            self.sets_within(SHELVED_SETS),
            self.followed(),
        )
        .await;
        Home::from_outcomes([
            history.map(|tracks| vec![Shelf::RecentlyPlayed(tracks.into())]),
            streamed.map(|tracks| vec![Shelf::Following(tracks.into())]),
            selections,
            playlists.map(|playlists| {
                vec![Shelf::YourPlaylists(
                    playlists.into_iter().map(Card::Playlist).collect(),
                )]
            }),
            followed.map(|artists| {
                vec![Shelf::Artists(
                    artists.into_iter().map(Card::Artist).collect(),
                )]
            }),
        ])
    }

    async fn search(&self, query: &str) -> Result<Found> {
        let (tracks, albums, artists, playlists) = try_join4(
            self.searched("tracks", query),
            self.searched("albums", query),
            self.searched("users", query),
            self.searched("playlists_without_albums", query),
        )
        .await?;
        Ok(Found {
            tracks: tracks.iter().filter_map(parse::track).collect(),
            albums: albums.iter().filter_map(parse::album).collect(),
            artists: artists.iter().filter_map(parse::artist).collect(),
            playlists: playlists
                .iter()
                .filter_map(|value| parse::playlist(value, &self.me))
                .collect(),
        })
    }

    async fn saved_tracks(&self) -> Result<Arc<[Track]>> {
        let liked = self
            .paginate(&format!("users/{}/track_likes", self.me), LIKED)
            .await?;
        Ok(liked
            .iter()
            .filter_map(|item| item.get("track"))
            .filter_map(parse::track)
            .collect())
    }

    async fn playlists(&self) -> Result<Arc<[Playlist]>> {
        Ok(self.sets_within(SETS).await?.into())
    }

    async fn playlist(&self, id: &Id) -> Result<PlaylistPage> {
        let set = self.set(id).await?;
        let playlist = parse::playlist(&set, &self.me)
            .ok_or_else(|| Error::Unreadable(format!("playlist {id}")))?;
        let tracks = self.listed(set).await?;
        Ok(PlaylistPage {
            playlist,
            tracks: tracks
                .iter()
                .filter_map(|(_, value)| parse::track(value))
                .collect(),
        })
    }

    async fn album(&self, id: &Id) -> Result<AlbumPage> {
        let set = self.set(id).await?;
        let album = parse::album(&set).ok_or_else(|| Error::Unreadable(format!("album {id}")))?;
        let tracks = self.listed(set).await?;
        Ok(AlbumPage {
            tracks: tracks
                .iter()
                .filter_map(|(number, value)| parse::in_album(value, &album, *number))
                .collect(),
            album,
        })
    }

    async fn artist(&self, id: &Id) -> Result<ArtistPage> {
        let limit = |most: usize| [("limit", most.to_string())];
        let (user, top, releases) = try_join3(
            self.api.get(&format!("users/{id}"), &[]),
            self.api
                .get(&format!("users/{id}/toptracks"), &limit(POPULAR)),
            self.api
                .get(&format!("users/{id}/albums"), &limit(RELEASES)),
        )
        .await?;
        let artist: Artist =
            parse::artist(&user).ok_or_else(|| Error::Unreadable(format!("artist {id}")))?;
        let mut popular: Vec<Track> = collection(&top).iter().filter_map(parse::track).collect();
        if popular.is_empty() {
            let uploads = self
                .api
                .get(&format!("users/{id}/tracks"), &limit(POPULAR))
                .await?;
            popular = collection(&uploads)
                .iter()
                .filter_map(parse::track)
                .collect();
        }
        let (singles, albums): (Vec<Album>, Vec<Album>) = collection(&releases)
            .iter()
            .filter_map(parse::album)
            .partition(|album| matches!(album.release, Release::Single | Release::Ep));
        Ok(ArtistPage {
            artist,
            popular: popular.into(),
            albums: albums.into(),
            singles: singles.into(),
        })
    }

    async fn mix(&self, seed: &Track) -> Result<Arc<[Track]>> {
        let answer = self
            .api
            .get(
                &format!("tracks/{}/related", seed.id),
                &[("limit", RELATED.to_string())],
            )
            .await?;
        let related = collection(&answer).iter().filter_map(parse::track);
        Ok(unique(std::iter::once(seed.clone()).chain(related)).into())
    }

    async fn set_saved(&self, track: &Id, saved: bool) -> Result<()> {
        let method = match saved {
            true => Method::PUT,
            false => Method::DELETE,
        };
        self.api
            .send(
                method,
                &format!("users/{}/track_likes/{track}", self.me),
                None,
            )
            .await
            .map(drop)
    }

    async fn create_playlist(&self, name: &str, description: &str) -> Result<Playlist> {
        let answer = self
            .api
            .send(
                Method::POST,
                "playlists",
                Some(&json!({
                    "playlist": {
                        "title": name,
                        "description": description,
                        "sharing": "private",
                        "tracks": [],
                    }
                })),
            )
            .await?;
        parse::playlist(&answer, &self.me)
            .ok_or_else(|| Error::Unreadable("the new playlist".into()))
    }

    async fn rename_playlist(&self, playlist: &Id, name: &str) -> Result<()> {
        self.api
            .send(
                Method::PUT,
                &format!("playlists/{playlist}"),
                Some(&json!({ "playlist": { "title": name } })),
            )
            .await
            .map(drop)
    }

    async fn delete_playlist(&self, playlist: &Playlist) -> Result<()> {
        let id = &playlist.id;
        let path = match (playlist.editable, id.as_str().starts_with(SYSTEM)) {
            (true, _) => format!("playlists/{id}"),
            (false, true) => format!("users/{}/system_playlist_likes/{id}", self.me),
            (false, false) => format!("users/{}/playlist_likes/{id}", self.me),
        };
        self.api.send(Method::DELETE, &path, None).await.map(drop)
    }

    async fn add_to_playlist(&self, playlist: &Id, tracks: &[Id]) -> Result<()> {
        let mut listed = self.track_ids(playlist).await?;
        for track in tracks {
            let track = numeric(track)?;
            if !listed.contains(&track) {
                listed.push(track);
            }
        }
        self.save_tracks(playlist, &listed).await
    }

    async fn remove_from_playlist(&self, playlist: &Id, track: &Id) -> Result<()> {
        let listed = self.track_ids(playlist).await?;
        let track = numeric(track)?;
        let remaining: Vec<u64> = listed.iter().copied().filter(|id| *id != track).collect();
        if remaining.len() == listed.len() {
            return Err(Error::Missing(format!("{track} in playlist {playlist}")));
        }
        self.save_tracks(playlist, &remaining).await
    }

    async fn arrange_playlist(&self, playlist: &Id, tracks: &[Id]) -> Result<()> {
        let listed = tracks.iter().map(numeric).collect::<Result<Vec<u64>>>()?;
        self.save_tracks(playlist, &listed).await
    }

    fn link(&self, shared: Shared<'_>) -> String {
        self.api.link(shared)
    }
}

fn numeric(track: &Id) -> Result<u64> {
    track
        .as_str()
        .parse()
        .map_err(|_| Error::Missing(format!("SoundCloud track {track}")))
}

fn unique(tracks: impl Iterator<Item = Track>) -> Vec<Track> {
    let mut seen = HashSet::new();
    tracks
        .filter(|track| seen.insert(track.id.clone()))
        .collect()
}
