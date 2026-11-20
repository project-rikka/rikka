use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::event::Events;
use crate::isrc::Isrc;
use crate::item::{Id, Playlist, Track};
use crate::page::{AlbumPage, ArtistPage, Found, Home, PlaylistPage};
use crate::quality::Quality;
use crate::service::Service;
use crate::web::WebSignIn;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shared<'a> {
    Track(&'a Id),
    Playlist(&'a Id),
}

#[derive(Debug)]
pub enum SignInStep {
    Browse(String),
    Web(WebSignIn),
}

pub type Prompt = Arc<dyn Fn(SignInStep) + Send + Sync>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
}

pub struct Link {
    pub account: Account,
    pub catalog: Arc<dyn Catalog>,
    pub playback: Arc<dyn Playback>,
}

impl std::fmt::Debug for Link {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Link")
            .field("account", &self.account)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct Connection {
    pub service: Service,
    pub account: Account,
    pub catalog: Arc<dyn Catalog>,
    pub playback: Arc<dyn Playback>,
}

impl std::fmt::Debug for Connection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Connection")
            .field("service", &self.service)
            .field("account", &self.account)
            .finish_non_exhaustive()
    }
}

#[async_trait]
pub trait Provider: Send + Sync {
    async fn resume(&self, folder: &Path) -> Result<Option<Link>>;

    async fn sign_in(&self, folder: &Path, prompt: Prompt) -> Result<Link>;
}

#[async_trait]
pub trait Catalog: Send + Sync {
    async fn home(&self) -> Result<Home>;

    async fn search(&self, query: &str) -> Result<Found>;

    async fn saved_tracks(&self) -> Result<Arc<[Track]>>;

    async fn playlists(&self) -> Result<Arc<[Playlist]>>;

    async fn playlist(&self, id: &Id) -> Result<PlaylistPage>;

    async fn album(&self, id: &Id) -> Result<AlbumPage>;

    async fn artist(&self, id: &Id) -> Result<ArtistPage>;

    async fn mix(&self, seed: &Track) -> Result<Arc<[Track]>>;

    async fn set_saved(&self, track: &Id, saved: bool) -> Result<()>;

    async fn create_playlist(&self, name: &str, description: &str) -> Result<Playlist>;

    async fn rename_playlist(&self, playlist: &Id, name: &str) -> Result<()>;

    async fn delete_playlist(&self, playlist: &Playlist) -> Result<()>;

    async fn add_to_playlist(&self, playlist: &Id, tracks: &[Id]) -> Result<()>;

    async fn remove_from_playlist(&self, playlist: &Id, track: &Id) -> Result<()>;

    async fn arrange_playlist(&self, playlist: &Id, tracks: &[Id]) -> Result<()>;

    async fn keeps_playlist(&self, _playlist: &Id) -> Result<bool> {
        Ok(true)
    }

    async fn lookup(&self, track: &Track) -> Result<Arc<[Track]>> {
        Ok(self.search(&query_for(track)).await?.tracks)
    }

    fn link(&self, shared: Shared<'_>) -> String;
}

fn query_for(track: &Track) -> String {
    match track.artists.first() {
        Some(artist) => format!("{} {}", artist.name, track.name),
        None => track.name.clone(),
    }
}

pub async fn lookup_by_isrc<F, S>(
    target: &Track,
    asking: impl FnOnce(&Isrc) -> String,
    search: S,
) -> Result<Arc<[Track]>>
where
    S: Fn(String) -> F,
    F: Future<Output = Result<Found>>,
{
    if let Some(isrc) = &target.isrc {
        let results = search(asking(isrc)).await?;
        let same: Arc<[Track]> = results
            .tracks
            .iter()
            .filter(|track| track.isrc.as_ref() == Some(isrc))
            .cloned()
            .collect();
        if !same.is_empty() {
            return Ok(same);
        }
    }
    Ok(search(query_for(target)).await?.tracks)
}

pub trait Playback: Send + Sync {
    fn load(&self, track: &Id, at: Duration, play: bool);

    fn prepare(&self, next: Option<&Id>);

    fn play(&self);

    fn pause(&self);

    fn seek(&self, at: Duration);

    fn set_quality(&self, quality: Quality);

    fn events(&self) -> Events;
}
