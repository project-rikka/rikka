use std::sync::Arc;

use futures_util::future::join4;
use music_core::{
    AlbumPage, ArtistPage, Card, Catalog, Error, Found, Home, Id, Playlist, PlaylistPage, Result,
    Shared, Track, async_trait,
};
use serde_json::{Value, json};

use crate::clients::Client;
use crate::editing;
use crate::home;
use crate::innertube::Innertube;
use crate::json::Walk;
use crate::pages;
use crate::parse::{self, LIKED};

const OPEN: &str = "https://music.youtube.com";
const HOME: &str = "FEmusic_home";
const LIBRARY_PLAYLISTS: &str = "FEmusic_liked_playlists";
const HOME_PAGES: usize = 4;
const PLAYLIST_PAGES: usize = 50;
const SONGS: &str = "EgWKAQIIAQ%3D%3D";
const ALBUMS: &str = "EgWKAQIYAQ%3D%3D";
const ARTISTS: &str = "EgWKAQIgAQ%3D%3D";
const PLAYLISTS: &str = "EgWKAQIoAQ%3D%3D";

pub(crate) struct YouTubeCatalog {
    tube: Arc<Innertube>,
    account: String,
}

impl YouTubeCatalog {
    pub(crate) fn new(tube: Arc<Innertube>, account: String) -> YouTubeCatalog {
        YouTubeCatalog { tube, account }
    }

    async fn browse(&self, browse_id: &str) -> Result<Value> {
        self.tube
            .call("browse", Client::Music, json!({ "browseId": browse_id }))
            .await
    }

    async fn carry_on(&self, token: &str) -> Result<Value> {
        self.tube
            .call("browse", Client::Music, json!({ "continuation": token }))
            .await
    }

    async fn searched(&self, query: &str, filter: &str) -> Result<Vec<Value>> {
        let answer = self
            .tube
            .call(
                "search",
                Client::Music,
                json!({ "query": query, "params": filter }),
            )
            .await?;
        Ok(answer
            .every("musicShelfRenderer")
            .into_iter()
            .flat_map(|shelf| shelf.list(&["contents"]).iter().cloned())
            .collect())
    }

    async fn whole_playlist(&self, id: &Id) -> Result<pages::Listed> {
        let answer = self.browse(&format!("VL{id}")).await?;
        let mut listed = pages::playlist(id, &answer, &self.account)?;
        for _ in 0..PLAYLIST_PAGES {
            let Some(token) = listed.more.take() else {
                break;
            };
            let from = u32::try_from(listed.tracks.len() + 1).unwrap_or(u32::MAX);
            let next = self.carry_on(&token).await?;
            let (tracks, more) = pages::more_tracks(&next, from);
            if tracks.is_empty() {
                break;
            }
            listed.tracks.extend(tracks);
            listed.slots.extend(pages::slots(&next));
            listed.more = more;
        }
        Ok(listed)
    }

    async fn edit(&self, body: Value) -> Result<()> {
        let answer = self.tube.call(editing::EDIT, Client::Music, body).await?;
        editing::check_status(&answer)
    }
}

#[async_trait]
impl Catalog for YouTubeCatalog {
    async fn home(&self) -> Result<Home> {
        let mut answer = self.browse(HOME).await?;
        let mut shelves = Vec::new();
        home::shelves(&answer, &mut shelves);
        for _ in 1..HOME_PAGES {
            let Some(token) = home::continuation(&answer) else {
                break;
            };
            answer = match self.carry_on(&token).await {
                Ok(next) => next,
                Err(error) => {
                    tracing::warn!("failed to load YouTube Music home continuation: {error}");
                    break;
                }
            };
            home::shelves(&answer, &mut shelves);
        }
        Ok(Home { shelves })
    }

    async fn search(&self, query: &str) -> Result<Found> {
        let (songs, albums, artists, playlists) = join4(
            self.searched(query, SONGS),
            self.searched(query, ALBUMS),
            self.searched(query, ARTISTS),
            self.searched(query, PLAYLISTS),
        )
        .await;
        if let (Err(error), Err(_), Err(_), Err(_)) = (&songs, &albums, &artists, &playlists) {
            return Err(error.clone());
        }
        let items = |result: Result<Vec<Value>>| result.unwrap_or_default();
        Ok(Found {
            tracks: items(songs)
                .iter()
                .zip(1..)
                .filter_map(|(item, number)| parse::list_track(item, number))
                .filter(|track| track.playable)
                .collect(),
            albums: items(albums).iter().filter_map(parse::list_album).collect(),
            artists: items(artists)
                .iter()
                .filter_map(parse::list_artist)
                .collect(),
            playlists: items(playlists)
                .iter()
                .filter_map(parse::list_playlist)
                .collect(),
        })
    }

    async fn saved_tracks(&self) -> Result<Arc<[Track]>> {
        Ok(self.whole_playlist(&Id::new(LIKED)).await?.tracks.into())
    }

    async fn playlists(&self) -> Result<Arc<[Playlist]>> {
        let answer = self.browse(LIBRARY_PLAYLISTS).await?;
        Ok(answer
            .every("musicTwoRowItemRenderer")
            .into_iter()
            .filter_map(parse::two_row)
            .filter_map(|card| match card {
                Card::Playlist(playlist) => Some(playlist),
                _ => None,
            })
            .map(|playlist| match playlist.owner.is_empty() {
                true => Playlist {
                    owner: self.account.clone(),
                    ..playlist
                },
                false => playlist,
            })
            .map(|playlist| Playlist {
                editable: playlist.owner == self.account,
                ..playlist
            })
            .collect())
    }

    async fn playlist(&self, id: &Id) -> Result<PlaylistPage> {
        let listed = self.whole_playlist(id).await?;
        Ok(PlaylistPage {
            playlist: listed.playlist,
            tracks: listed.tracks.into(),
        })
    }

    async fn album(&self, id: &Id) -> Result<AlbumPage> {
        pages::album(id, &self.browse(id.as_str()).await?)
    }

    async fn artist(&self, id: &Id) -> Result<ArtistPage> {
        pages::artist(id, &self.browse(id.as_str()).await?)
    }

    async fn mix(&self, seed: &Track) -> Result<Arc<[Track]>> {
        let answer = self
            .tube
            .call(
                "next",
                Client::Music,
                json!({
                    "videoId": seed.id.as_str(),
                    "playlistId": format!("RDAMVM{}", seed.id),
                    "enablePersistentPlaylistPanel": true,
                    "tunerSettingValue": "AUTOMIX_SETTING_NORMAL",
                }),
            )
            .await?;
        let tracks: Arc<[Track]> = answer
            .every("playlistPanelRenderer")
            .into_iter()
            .flat_map(|panel| panel.list(&["contents"]))
            .zip(1..)
            .filter_map(|(item, number)| parse::panel_track(item, number))
            .collect();
        match tracks.is_empty() {
            true => Err(Error::Missing(format!("a mix of {}", seed.name))),
            false => Ok(tracks),
        }
    }

    async fn set_saved(&self, track: &Id, saved: bool) -> Result<()> {
        let endpoint = match saved {
            true => "like/like",
            false => "like/removelike",
        };
        self.tube
            .call(
                endpoint,
                Client::Music,
                json!({ "target": { "videoId": track.as_str() } }),
            )
            .await
            .map(drop)
    }

    async fn create_playlist(&self, name: &str, description: &str) -> Result<Playlist> {
        let answer = self
            .tube
            .call(
                editing::CREATE,
                Client::Music,
                editing::created(name, description),
            )
            .await?;
        Ok(Playlist {
            id: editing::made(&answer)?,
            name: name.to_owned(),
            owner: self.account.clone(),
            description: description.to_owned(),
            artwork: Default::default(),
            length: 0,
            editable: true,
        })
    }

    async fn rename_playlist(&self, playlist: &Id, name: &str) -> Result<()> {
        self.edit(editing::renamed(playlist, name)).await
    }

    async fn delete_playlist(&self, playlist: &Playlist) -> Result<()> {
        let (endpoint, body) = match playlist.editable {
            true => (editing::DELETE, editing::named(&playlist.id)),
            false => (editing::UNSAVE, editing::unsaved(&playlist.id)),
        };
        self.tube
            .call(endpoint, Client::Music, body)
            .await
            .map(drop)
    }

    async fn add_to_playlist(&self, playlist: &Id, tracks: &[Id]) -> Result<()> {
        self.edit(editing::added(playlist, tracks)).await
    }

    async fn remove_from_playlist(&self, playlist: &Id, track: &Id) -> Result<()> {
        let listed = self.whole_playlist(playlist).await?;
        self.edit(editing::removed(playlist, &listed.slots, track)?)
            .await
    }

    async fn arrange_playlist(&self, playlist: &Id, tracks: &[Id]) -> Result<()> {
        let listed = self.whole_playlist(playlist).await?;
        for batch in tracks.chunks(editing::BATCH) {
            self.edit(editing::appended(playlist, batch)).await?;
        }
        for slots in listed.slots.chunks(editing::BATCH) {
            self.edit(editing::cleared(playlist, slots)).await?;
        }
        Ok(())
    }

    fn link(&self, shared: Shared<'_>) -> String {
        match shared {
            Shared::Track(id) => format!("{OPEN}/watch?v={id}"),
            Shared::Playlist(id) => format!("{OPEN}/playlist?list={id}"),
        }
    }
}
