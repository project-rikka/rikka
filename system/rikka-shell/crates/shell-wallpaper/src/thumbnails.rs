use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::SystemTime;

use rikka_image::Image;
use rikka_session::wallpaper::Thumbnail;
use rikka_sync::Mutex;
use rikka_thumbnails::{Cache, Size};

use crate::entry::Entry;
use crate::error::{Error, Result};
use crate::kind::Kind;

const SIZE: Size = Size::XLarge;

pub(super) type Announce = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone)]
pub(super) struct Thumbnails {
    cache: Cache,
    queue_state: Arc<Mutex<ThumbnailQueue>>,
    announce: Arc<OnceLock<Announce>>,
}

impl std::fmt::Debug for Thumbnails {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Thumbnails")
            .field("cache", &self.cache)
            .field("making", &self.queue_state)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Default)]
struct ThumbnailQueue {
    queue: VecDeque<ThumbnailJob>,
    requested: HashSet<PathBuf>,
    refused: HashSet<PathBuf>,
    running: bool,
}

#[derive(Debug)]
struct ThumbnailJob {
    entry: Entry,
    place: Place,
}

#[derive(Clone, Debug)]
struct Place {
    path: PathBuf,
    uri: String,
    modified: SystemTime,
    made: bool,
}

impl Place {
    #[must_use]
    fn for_entry(cache: &Cache, entry: &Entry) -> Option<Place> {
        if entry.kind() != Kind::Still {
            return None;
        }
        let modified = std::fs::metadata(entry.path()).ok()?.modified().ok()?;
        let uri = rikka_open::path_to_uri(entry.path());
        let path = cache.path_of(&uri, SIZE);
        let made = Cache::describing(&path, &uri, modified).is_some();
        Some(Place {
            path,
            uri,
            modified,
            made,
        })
    }
}

impl Thumbnails {
    #[must_use]
    pub(crate) fn shared() -> Option<Thumbnails> {
        Some(Thumbnails {
            cache: Cache::shared()?,
            queue_state: Arc::default(),
            announce: Arc::default(),
        })
    }

    pub(crate) fn announcing(&self, announce: Announce) {
        if self.announce.set(announce).is_err() {
            tracing::warn!("wallpaper thumbnail announcer already set");
        }
    }

    pub(crate) fn settle(&self, entries: &[Entry]) -> Vec<Thumbnail> {
        let places: Vec<Option<Place>> = entries
            .iter()
            .map(|entry| Place::for_entry(&self.cache, entry))
            .collect();
        let mut queue_state = self.queue_state.lock();
        let current: HashSet<&Path> = places
            .iter()
            .flatten()
            .map(|place| place.path.as_path())
            .collect();
        queue_state
            .refused
            .retain(|path| current.contains(path.as_path()));
        let small = entries
            .iter()
            .zip(&places)
            .map(|(entry, place)| {
                let Some(place) = place else {
                    return Thumbnail::None;
                };
                if place.made {
                    return Thumbnail::Made {
                        file: place.path.clone(),
                    };
                }
                if queue_state.refused.contains(&place.path) {
                    return Thumbnail::None;
                }
                if queue_state.requested.insert(place.path.clone()) {
                    queue_state.queue.push_back(ThumbnailJob {
                        entry: entry.clone(),
                        place: place.clone(),
                    });
                }
                Thumbnail::Making
            })
            .collect();
        if !queue_state.running && !queue_state.queue.is_empty() {
            queue_state.running = true;
            drop(queue_state);
            self.start();
        }
        small
    }

    fn start(&self) {
        let (cache, queue_state, announce) = (
            self.cache.clone(),
            Arc::clone(&self.queue_state),
            Arc::clone(&self.announce),
        );
        let spawned = std::thread::Builder::new()
            .name("rikka-wallpaper-thumbnails".to_owned())
            .spawn(move || {
                while let Some(job) = next(&queue_state) {
                    if let Err(error) = make(&cache, &job) {
                        tracing::warn!(
                            "failed to make thumbnail for {}: {error}",
                            job.entry.path().display()
                        );
                        queue_state.lock().refused.insert(job.place.path.clone());
                    }
                    queue_state.lock().requested.remove(&job.place.path);
                    if let Some(announce) = announce.get() {
                        announce();
                    }
                }
            });
        if let Err(error) = spawned {
            tracing::warn!("cannot spawn wallpaper thumbnail thread: {error}");
            self.queue_state.lock().running = false;
        }
    }
}

fn next(queue_state: &Mutex<ThumbnailQueue>) -> Option<ThumbnailJob> {
    let mut queue_state = queue_state.lock();
    let job = queue_state.queue.pop_front();
    if job.is_none() {
        queue_state.running = false;
    }
    job
}

fn make(cache: &Cache, job: &ThumbnailJob) -> Result<()> {
    let picture = job.entry.path();
    let side = SIZE.pixels();
    let thumbnail = Image::open(picture)
        .and_then(|image| image.thumbnail(side, side))
        .map_err(|source| Error::Thumbnail {
            path: picture.to_path_buf(),
            source,
        })?;
    let bytes = std::fs::metadata(picture).map(|meta| meta.len()).ok();
    let place = &job.place;
    cache
        .store(
            &place.path,
            thumbnail,
            &Cache::fields(&place.uri, place.modified, bytes),
        )
        .map_err(|source| Error::Write {
            path: place.path.clone(),
            source,
        })
}
