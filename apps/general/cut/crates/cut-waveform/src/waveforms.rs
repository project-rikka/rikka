use rikka_sync::{Condvar, Mutex, RwLock};
use std::collections::{HashMap, VecDeque};
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rikka_worker::{Stop, Worker};

use crate::error::{Error, Result};
use crate::file::{Fingerprint, decode, encode};
use crate::peaks::Peaks;

const DECODERS: [&str; 2] = ["rikka-cut-waveform-0", "rikka-cut-waveform-1"];
const CHUNK: usize = 1 << 15;
const NOTIFY_INTERVAL: Duration = Duration::from_millis(150);

type Listener = Arc<dyn Fn() + Send + Sync>;

pub struct Waveforms {
    inner: Arc<Inner>,
    decoders: Vec<Worker<Arc<Inner>>>,
}

struct Inner {
    cache: Option<PathBuf>,
    closed: AtomicBool,
    queue: Mutex<Queue>,
    queued: Condvar,
    listener: Mutex<Option<Listener>>,
}

#[derive(Default)]
struct Queue {
    known: HashMap<PathBuf, Waveform>,
    waiting: VecDeque<Waveform>,
}

#[derive(Clone)]
pub struct Waveform(Arc<Shared>);

struct Shared {
    path: PathBuf,
    state: RwLock<State>,
}

#[derive(Default)]
struct State {
    peaks: Option<Peaks>,
    failure: Option<String>,
}

impl Waveforms {
    pub fn new(cache: Option<PathBuf>) -> Result<Waveforms> {
        let inner = Arc::new(Inner {
            cache,
            closed: AtomicBool::new(false),
            queue: Mutex::new(Queue::default()),
            queued: Condvar::new(),
            listener: Mutex::new(None),
        });
        let decoders = DECODERS
            .into_iter()
            .map(|name| {
                let decoder = Arc::clone(&inner);
                Worker::spawn(name, Arc::clone(&inner), move || {
                    Ok::<_, Infallible>(move || decoder.work())
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(Error::Spawn)?;
        Ok(Waveforms { inner, decoders })
    }

    pub fn on_change(&self, listener: impl Fn() + Send + Sync + 'static) {
        *self.inner.listener.lock() = Some(Arc::new(listener));
    }

    #[must_use]
    pub fn waveform(&self, path: &Path) -> Waveform {
        let mut queue = self.inner.queue.lock();
        if let Some(known) = queue.known.get(path) {
            return known.clone();
        }
        let waveform = Waveform(Arc::new(Shared {
            path: path.to_path_buf(),
            state: RwLock::new(State::default()),
        }));
        queue.known.insert(path.to_path_buf(), waveform.clone());
        queue.waiting.push_back(waveform.clone());
        drop(queue);
        self.inner.queued.notify_one();
        waveform
    }

    pub fn forget(&self, path: &Path) {
        let mut queue = self.inner.queue.lock();
        queue.known.remove(path);
        queue.waiting.retain(|waiting| waiting.path() != path);
    }
}

impl std::fmt::Debug for Waveforms {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Waveforms")
            .field("cache", &self.inner.cache)
            .field("decoders", &self.decoders.len())
            .field("known", &self.inner.queue.lock().known.len())
            .finish_non_exhaustive()
    }
}

impl Stop for Inner {
    fn stop(&self) {
        self.closed.store(true, Ordering::Release);
        let _queue = self.queue.lock();
        self.queued.notify_all();
    }
}

impl Inner {
    fn work(&self) {
        while let Some(waveform) = self.next() {
            if let Err(error) = self.read(&waveform) {
                tracing::debug!(%error, path = %waveform.0.path.display(), "failed to read waveform");
                waveform.write().failure = Some(error.to_string());
            }
            self.notify();
        }
    }

    fn next(&self) -> Option<Waveform> {
        let mut queue = self.queue.lock();
        loop {
            if self.closed.load(Ordering::Acquire) {
                return None;
            }
            if let Some(next) = queue.waiting.pop_front() {
                return Some(next);
            }
            queue = self.queued.wait(queue);
        }
    }

    fn read(&self, waveform: &Waveform) -> Result<()> {
        let path = &waveform.0.path;
        let fingerprint = Fingerprint::read(path)?;
        let cached = self
            .cache
            .as_ref()
            .map(|folder| folder.join(fingerprint.cache_name()));
        if let Some(peaks) = cached
            .as_deref()
            .and_then(|file| read_cached(&fingerprint, file))
        {
            waveform.write().peaks = Some(peaks);
            return Ok(());
        }

        let mut decoder = rikka_audio_decode::read(path).decoder()?;
        let channels = usize::from(decoder.channels().max(1));
        waveform.write().peaks = Some(
            Peaks::new(decoder.sample_rate(), decoder.channels())
                .with_expected_frames(decoder.frames()),
        );
        self.notify();
        let mut decoded = Vec::new();
        let mut notified = Instant::now();
        loop {
            if self.closed.load(Ordering::Acquire) {
                return Ok(());
            }
            let more = decoder.read(&mut decoded)?;
            if decoded.len() >= CHUNK * channels || !more {
                let whole = decoded.len() / channels * channels;
                if let Some(peaks) = waveform.write().peaks.as_mut() {
                    peaks.extend(&decoded[..whole]);
                    if !more {
                        peaks.finish();
                    }
                }
                decoded.clear();
                if notified.elapsed() >= NOTIFY_INTERVAL {
                    notified = Instant::now();
                    self.notify();
                }
            }
            if !more {
                break;
            }
        }

        let encoded = cached.zip(waveform.with_peaks(|peaks| encode(&fingerprint, peaks)));
        if let Some((file, bytes)) = encoded {
            let result = file
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| rikka_fs::replace(&file, bytes));
            if let Err(error) = result {
                tracing::debug!(%error, path = %file.display(), "failed to write waveform cache");
            }
        }
        Ok(())
    }

    fn notify(&self) {
        if let Some(listener) = self.listener.lock().clone() {
            listener();
        }
    }
}

fn read_cached(fingerprint: &Fingerprint, file: &Path) -> Option<Peaks> {
    let bytes = std::fs::read(file).ok()?;
    match decode(fingerprint, &bytes) {
        Ok(peaks) => Some(peaks),
        Err(error) => {
            tracing::debug!(%error, path = %file.display(), "discarding waveform cache");
            None
        }
    }
}

impl Waveform {
    pub fn with_peaks<R>(&self, read: impl FnOnce(&Peaks) -> R) -> Option<R> {
        let state = self.0.state.read();
        state.peaks.as_ref().map(read)
    }

    #[must_use]
    pub fn failure(&self) -> Option<String> {
        let state = self.0.state.read();
        state.failure.clone()
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0.path
    }

    fn write(&self) -> rikka_sync::RwLockWriteGuard<'_, State> {
        self.0.state.write()
    }
}

impl std::fmt::Debug for Waveform {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Waveform")
            .field("path", &self.0.path)
            .finish_non_exhaustive()
    }
}
