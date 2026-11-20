use rikka_sync::{Condvar, Mutex};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::convert::Infallible;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rikka_video::{PixelSize, Stills};
use rikka_worker::{Stop, Worker};

use crate::error::Result;

const WORKERS: [&str; 2] = ["rikka-cut-filmstrip-0", "rikka-cut-filmstrip-1"];
const MAX_OPEN_FILES: usize = 3;
const LOOKAHEAD: usize = 24;
const CACHE_BUDGET: usize = 192 << 20;
const HEIGHTS: [u32; 9] = [24, 32, 45, 64, 90, 128, 181, 256, 362];
const MAX_ASPECT: u32 = 4;

type Listener = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Request {
    pub path: Arc<Path>,
    pub at: Duration,
    pub tolerance: Duration,
    pub height: u32,
}

#[derive(Clone)]
pub struct Thumbnail {
    pub pts: Duration,
    pub width: u32,
    pub height: u32,
    pub pixels: Arc<[u8]>,
}

impl std::fmt::Debug for Thumbnail {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Thumbnail")
            .field("pts", &self.pts)
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub struct Match {
    pub thumbnail: Thumbnail,
    pub exact: bool,
}

pub struct Filmstrips {
    inner: Arc<Inner>,
    workers: Vec<Worker<Arc<Inner>>>,
}

struct Inner {
    state: Mutex<State>,
    changed: Condvar,
    listener: Mutex<Option<Listener>>,
}

#[derive(Default)]
struct State {
    media: HashMap<Arc<Path>, MediaCache>,
    requests: Vec<Request>,
    busy: HashSet<Key>,
    failed_media: HashSet<Arc<Path>>,
    failed_frames: HashSet<Key>,
    bytes: usize,
    clock: u64,
    closed: bool,
}

#[derive(Default)]
struct MediaCache {
    sizes: BTreeMap<u32, BTreeMap<Duration, Cached>>,
}

struct Cached {
    thumbnail: Thumbnail,
    tolerance: Duration,
    used: u64,
}

impl Cached {
    fn satisfies(&self, request: &Request) -> bool {
        self.tolerance <= request.tolerance
            || (request.at..=request.at.saturating_add(request.tolerance))
                .contains(&self.thumbnail.pts)
    }
}

enum Failure {
    Open(rikka_video::Error),
    Decode(rikka_video::Error),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    path: Arc<Path>,
    at: Duration,
    height: u32,
}

#[must_use]
pub fn decoded_height(display_height: f32) -> u32 {
    let needed = display_height.max(1.0).ceil() as u32;
    HEIGHTS
        .into_iter()
        .find(|step| *step >= needed)
        .unwrap_or(HEIGHTS[HEIGHTS.len() - 1])
}

impl Filmstrips {
    pub fn new() -> Result<Filmstrips> {
        let inner = Arc::new(Inner {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
            listener: Mutex::new(None),
        });
        let workers = WORKERS
            .into_iter()
            .map(|name| {
                let worker = Arc::clone(&inner);
                Worker::spawn(name, Arc::clone(&inner), move || {
                    Ok::<_, Infallible>(move || worker.work())
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Filmstrips { inner, workers })
    }

    pub fn on_change(&self, listener: impl Fn() + Send + Sync + 'static) {
        *self.inner.listener.lock() = Some(Arc::new(listener));
    }

    pub fn set_requests(&self, requests: Vec<Request>) {
        let mut state = self.inner.state.lock();
        if state.requests != requests {
            state.requests = requests;
            self.inner.changed.notify_all();
        }
    }

    pub fn forget(&self, path: &Path) {
        let mut state = self.inner.state.lock();
        state.failed_media.remove(path);
        state.failed_frames.retain(|key| *key.path != *path);
        if let Some(cache) = state.media.remove(path) {
            let freed: usize = cache
                .sizes
                .values()
                .flat_map(BTreeMap::values)
                .map(|cached| cached.thumbnail.pixels.len())
                .sum();
            state.bytes -= freed;
        }
        drop(state);
        self.inner.changed.notify_all();
    }

    #[must_use]
    pub fn find(&self, request: &Request) -> Option<Match> {
        let mut state = self.inner.state.lock();
        state.clock += 1;
        let clock = state.clock;
        let cache = state.media.get_mut(&request.path)?;
        if let Some(cached) = cache
            .sizes
            .get_mut(&request.height)
            .and_then(|cached| cached.get_mut(&request.at))
        {
            cached.used = clock;
            let exact = cached.satisfies(request);
            return Some(Match {
                thumbnail: cached.thumbnail.clone(),
                exact,
            });
        }
        let low = request.at.saturating_sub(request.tolerance);
        let high = request.at.saturating_add(request.tolerance);
        cache
            .sizes
            .iter()
            .rev()
            .filter_map(|(_, cached)| {
                cached
                    .range(low..=high)
                    .min_by_key(|(at, _)| at.abs_diff(request.at))
                    .map(|(_, cached)| cached)
            })
            .min_by_key(|cached| cached.thumbnail.pts.abs_diff(request.at))
            .map(|cached| Match {
                thumbnail: cached.thumbnail.clone(),
                exact: false,
            })
    }
}

impl Stop for Inner {
    fn stop(&self) {
        self.state.lock().closed = true;
        self.changed.notify_all();
    }
}

impl std::fmt::Debug for Filmstrips {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.inner.state.lock();
        formatter
            .debug_struct("Filmstrips")
            .field("workers", &self.workers.len())
            .field("media", &state.media.len())
            .field("bytes", &state.bytes)
            .finish_non_exhaustive()
    }
}

struct OpenFile {
    path: Arc<Path>,
    stills: Stills,
    height: u32,
    at: Duration,
}

impl Inner {
    fn work(&self) {
        let mut open: Vec<OpenFile> = Vec::new();
        while let Some(request) = self.next(&open) {
            let key = Key {
                path: Arc::clone(&request.path),
                at: request.at,
                height: request.height,
            };
            let decoded = self.decode(&mut open, &request);
            let mut state = self.state.lock();
            state.busy.remove(&key);
            match decoded {
                Ok(thumbnail) => state.insert(key, thumbnail, request.tolerance),
                Err(Failure::Open(error)) => {
                    tracing::debug!(%error, path = %request.path.display(), "cannot open video for thumbnails");
                    state.failed_media.insert(Arc::clone(&request.path));
                }
                Err(Failure::Decode(error)) => {
                    tracing::debug!(%error, path = %request.path.display(), at = ?request.at, "cannot decode thumbnail frame");
                    open.retain(|file| file.path != request.path);
                    state.failed_frames.insert(key);
                }
            }
            drop(state);
            if let Some(listener) = self.listener.lock().clone() {
                listener();
            }
        }
    }

    fn next(&self, open: &[OpenFile]) -> Option<Request> {
        let mut state = self.state.lock();
        loop {
            if state.closed {
                return None;
            }
            if let Some(request) = state.next_request(open) {
                state.busy.insert(Key {
                    path: Arc::clone(&request.path),
                    at: request.at,
                    height: request.height,
                });
                return Some(request);
            }
            state = self.changed.wait(state);
        }
    }

    fn decode(&self, open: &mut Vec<OpenFile>, request: &Request) -> Result<Thumbnail, Failure> {
        let at = match open.iter().position(|file| file.path == request.path) {
            Some(at) => at,
            None => {
                if open.len() >= MAX_OPEN_FILES {
                    open.remove(0);
                }
                open.push(OpenFile {
                    path: Arc::clone(&request.path),
                    stills: Stills::open(&request.path).map_err(Failure::Open)?,
                    height: 0,
                    at: Duration::ZERO,
                });
                open.len() - 1
            }
        };
        let file = &mut open[at];
        if file.height != request.height {
            file.height = request.height;
            file.stills
                .set_max_size(PixelSize::new(request.height * MAX_ASPECT, request.height));
        }
        let still = file
            .stills
            .still(request.at, request.tolerance)
            .map_err(Failure::Decode)?;
        file.at = request.at;
        let file = open.remove(at);
        open.push(file);
        let width = still.image.width();
        let height = still.image.height();
        Ok(Thumbnail {
            pts: still.pts,
            width,
            height,
            pixels: Arc::from(still.image.into_rgba8()),
        })
    }
}

impl State {
    fn next_request(&self, open: &[OpenFile]) -> Option<Request> {
        let pending: Vec<&Request> = self
            .requests
            .iter()
            .filter(|request| {
                !self.failed_media.contains(&request.path) && !self.is_satisfied(request)
            })
            .take(LOOKAHEAD)
            .collect();
        let sequential = open.last().and_then(|file| {
            pending
                .iter()
                .filter(|request| request.path == file.path && request.at >= file.at)
                .min_by_key(|request| request.at)
        });
        sequential
            .or(pending.first())
            .map(|request| (*request).clone())
    }

    fn is_satisfied(&self, request: &Request) -> bool {
        let key = Key {
            path: Arc::clone(&request.path),
            at: request.at,
            height: request.height,
        };
        self.busy.contains(&key)
            || self.failed_frames.contains(&key)
            || self
                .media
                .get(&request.path)
                .and_then(|cache| cache.sizes.get(&request.height))
                .and_then(|cached| cached.get(&request.at))
                .is_some_and(|cached| cached.satisfies(request))
    }

    fn insert(&mut self, key: Key, thumbnail: Thumbnail, tolerance: Duration) {
        self.clock += 1;
        self.bytes += thumbnail.pixels.len();
        let cached = Cached {
            thumbnail,
            tolerance,
            used: self.clock,
        };
        let replaced = self
            .media
            .entry(key.path)
            .or_default()
            .sizes
            .entry(key.height)
            .or_default()
            .insert(key.at, cached);
        if let Some(replaced) = replaced {
            self.bytes -= replaced.thumbnail.pixels.len();
        }
        if self.bytes > CACHE_BUDGET {
            self.evict();
        }
    }

    fn evict(&mut self) {
        let mut ages: Vec<(u64, Arc<Path>, u32, Duration, usize)> = self
            .media
            .iter()
            .flat_map(|(path, cache)| {
                cache.sizes.iter().flat_map(move |(height, cached)| {
                    cached.iter().map(move |(at, cached)| {
                        (
                            cached.used,
                            Arc::clone(path),
                            *height,
                            *at,
                            cached.thumbnail.pixels.len(),
                        )
                    })
                })
            })
            .collect();
        ages.sort_unstable_by_key(|age| age.0);
        let target = CACHE_BUDGET / 10 * 9;
        for (_, path, height, at, bytes) in ages {
            if self.bytes <= target {
                break;
            }
            if let Some(cache) = self.media.get_mut(&path)
                && let Some(cached) = cache.sizes.get_mut(&height)
                && cached.remove(&at).is_some()
            {
                self.bytes -= bytes;
            }
        }
        self.media.retain(|_, cache| {
            cache.sizes.retain(|_, cached| !cached.is_empty());
            !cache.sizes.is_empty()
        });
    }
}
