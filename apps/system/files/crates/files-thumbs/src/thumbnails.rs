use rikka_sync::{Condvar, Mutex};
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use files_core::{Entry, Family, Location};
use rikka_mime::MimeType;
use rikka_thumbnails::{Cache, Size};
use rikka_types::Size as Pixels;

use crate::external::Thumbnailers;
use crate::generate::{self, APPLICATION};

const DRAWING_EXTENSIONS: [&str; 2] = ["svg", "svgz"];

fn workers() -> usize {
    std::thread::available_parallelism()
        .map(|cores| cores.get().saturating_sub(2).clamp(1, 6))
        .unwrap_or(1)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Thumbnail {
    path: PathBuf,
    pixels: Pixels,
    drawing: Option<PathBuf>,
}

impl Thumbnail {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn shown(&self) -> &Path {
        self.drawing.as_deref().unwrap_or(&self.path)
    }

    #[must_use]
    pub fn pixels(&self) -> Pixels {
        self.pixels
    }
}

#[derive(Clone, Debug)]
struct Request {
    at: Location,
    mime: MimeType,
    modified: SystemTime,
    size: Size,
}

#[derive(Debug, Default)]
struct Queue {
    waiting: VecDeque<Request>,
    queued: HashSet<Location>,
    working: HashSet<Location>,
    idle: usize,
    started: usize,
    stopping: bool,
}

#[derive(Debug)]
pub struct Thumbnails {
    shared: Arc<Shared>,
    limit: usize,
}

struct Shared {
    queue: Mutex<Queue>,
    work: Condvar,
    cache: Cache,
    thumbnailers: Thumbnailers,
    sink: Sink,
}

pub type Sink = Arc<dyn Fn(Location) + Send + Sync>;

impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shared")
            .field("cache", &self.cache)
            .field("thumbnailers", &self.thumbnailers)
            .finish_non_exhaustive()
    }
}

impl Shared {
    fn ready(&self, at: &Location, modified: SystemTime, size: Size) -> Option<Thumbnail> {
        let path = self.cache.path_of(at.as_str(), size);
        let pixels = Cache::describing(&path, at.as_str(), modified)?;
        let drawing = at.to_path().filter(|original| is_drawing(original));
        Some(Thumbnail {
            path,
            pixels,
            drawing,
        })
    }

    fn known_bad(&self, at: &Location, modified: SystemTime) -> bool {
        let marker = self.cache.failure_path(at.as_str(), APPLICATION);
        Cache::describing(&marker, at.as_str(), modified).is_some()
    }
}

impl Thumbnails {
    #[must_use]
    pub fn new(cache: Cache, sink: Sink) -> Self {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::default()),
            work: Condvar::new(),
            cache,
            thumbnailers: Thumbnailers::load(),
            sink,
        });
        Self {
            shared,
            limit: workers(),
        }
    }

    fn grow(&self, index: usize) {
        let shared = Arc::clone(&self.shared);
        let spawned = std::thread::Builder::new()
            .name(format!("rikka-files-thumbs-{index}"))
            .spawn(move || work(&shared));
        if spawned.is_err() {
            self.shared.queue.lock().started -= 1;
        }
    }

    #[must_use]
    pub fn ready(&self, at: &Location, modified: SystemTime, size: Size) -> Option<Thumbnail> {
        self.shared.ready(at, modified, size)
    }

    #[must_use]
    pub fn known_bad(&self, at: &Location, modified: SystemTime) -> bool {
        self.shared.known_bad(at, modified)
    }

    #[must_use]
    pub fn can_thumbnail(&self, entry: &Entry) -> bool {
        if entry.is_directory() || entry.size == 0 {
            return false;
        }
        if entry
            .location
            .to_path()
            .is_some_and(|path| self.shared.cache.holds(&path))
        {
            return false;
        }
        Family::from_mime(&entry.mime) == Family::Image
            || self.shared.thumbnailers.handles(&entry.mime)
    }

    pub fn request(&self, entry: &Entry, size: Size) {
        let Some(modified) = entry.modified else {
            return;
        };
        if !self.can_thumbnail(entry) {
            return;
        }
        let slot = {
            let mut queue = self.shared.queue.lock();
            if queue.working.contains(&entry.location)
                || !queue.queued.insert(entry.location.clone())
            {
                return;
            }
            queue.waiting.push_front(Request {
                at: entry.location.clone(),
                mime: entry.mime.clone(),
                modified,
                size,
            });
            (queue.idle == 0 && queue.started < self.limit).then(|| {
                queue.started += 1;
                queue.started - 1
            })
        };

        self.shared.work.notify_one();
        if let Some(index) = slot {
            self.grow(index);
        }
    }

    pub fn retain(&self, visible: &HashSet<Location>) {
        let mut queue = self.shared.queue.lock();
        queue
            .waiting
            .retain(|request| visible.contains(&request.at));
        queue.queued.retain(|at| visible.contains(at));
    }
}

impl Drop for Thumbnails {
    fn drop(&mut self) {
        {
            let mut queue = self.shared.queue.lock();
            queue.stopping = true;
            queue.waiting.clear();
        }
        self.shared.work.notify_all();
    }
}

fn work(shared: &Arc<Shared>) {
    loop {
        let request = {
            let mut queue = shared.queue.lock();
            while queue.waiting.is_empty() && !queue.stopping {
                queue.idle += 1;
                queue = shared.work.wait(queue);
                queue.idle -= 1;
            }
            if queue.stopping {
                return;
            }
            let Some(request) = queue.waiting.pop_front() else {
                continue;
            };
            queue.queued.remove(&request.at);
            queue.working.insert(request.at.clone());
            request
        };

        let cached = shared
            .ready(&request.at, request.modified, request.size)
            .is_some()
            || shared.known_bad(&request.at, request.modified);
        if !cached {
            generate::make(
                &shared.cache,
                &shared.thumbnailers,
                &request.at,
                &request.mime,
                request.modified,
                request.size,
            );
        }
        let stopping = {
            let mut queue = shared.queue.lock();
            queue.working.remove(&request.at);
            queue.stopping
        };
        if !stopping && !cached {
            (shared.sink)(request.at);
        }
    }
}

fn is_drawing(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            DRAWING_EXTENSIONS
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
}
