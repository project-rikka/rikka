use rikka_sync::{Mutex, MutexGuard};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use futures_util::Stream;
use rikka_types::Size;
use rikka_worker::Observable;

use crate::consume::VideoConsumer;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SIZE_UNKNOWN: u64 = u64::MAX;

type Listener = dyn Fn(VideoEvent) + Send + Sync;

#[derive(Clone)]
pub struct VideoController(Arc<Shared>);

#[derive(Clone)]
pub struct WeakVideoController(Weak<Shared>);

#[must_use]
pub struct VideoWatch {
    _listener: Arc<Listener>,
}

impl WeakVideoController {
    #[must_use]
    pub fn upgrade(&self) -> Option<VideoController> {
        self.0.upgrade().map(VideoController)
    }

    #[must_use]
    pub fn is_dropped(&self) -> bool {
        self.0.strong_count() == 0
    }
}

impl std::fmt::Debug for WeakVideoController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WeakVideoController")
            .field("alive", &!self.is_dropped())
            .finish()
    }
}

impl std::fmt::Debug for VideoWatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoWatch").finish_non_exhaustive()
    }
}

struct Shared {
    id: u64,
    path: PathBuf,
    commands: Mutex<Vec<VideoCommand>>,
    size: AtomicU64,
    error: Mutex<Option<String>>,
    position_us: AtomicI64,
    duration_us: AtomicI64,
    state: AtomicU8,
    changed: Observable<()>,
    listeners: Mutex<Vec<Weak<Listener>>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VideoCommand {
    Play,
    Pause,
    Seek(Duration),
    SetLooping(bool),
    SetSpeed(f32),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum VideoState {
    #[default]
    Opening,
    Playing,
    Paused,
    Ended,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VideoEvent {
    Changed,
    Progressed,
}

impl VideoState {
    fn code(self) -> u8 {
        self as u8
    }

    fn from_code(code: u8) -> VideoState {
        match code {
            1 => VideoState::Playing,
            2 => VideoState::Paused,
            3 => VideoState::Ended,
            4 => VideoState::Failed,
            _ => VideoState::Opening,
        }
    }
}

impl VideoController {
    #[must_use]
    pub fn open(path: impl Into<PathBuf>) -> VideoController {
        VideoController(Arc::new(Shared {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            path: path.into(),
            commands: Mutex::new(Vec::new()),
            size: AtomicU64::new(SIZE_UNKNOWN),
            error: Mutex::new(None),
            position_us: AtomicI64::new(0),
            duration_us: AtomicI64::new(-1),
            state: AtomicU8::new(VideoState::Opening.code()),
            changed: Observable::default(),
            listeners: Mutex::new(Vec::new()),
        }))
    }

    #[must_use]
    pub fn downgrade(&self) -> WeakVideoController {
        WeakVideoController(Arc::downgrade(&self.0))
    }

    pub fn play(&self) {
        self.0.push(VideoCommand::Play);
    }

    pub fn pause(&self) {
        self.0.push(VideoCommand::Pause);
    }

    pub fn seek(&self, position: Duration) {
        self.0.push(VideoCommand::Seek(position));
    }

    pub fn set_looping(&self, looping: bool) {
        self.0.push(VideoCommand::SetLooping(looping));
    }

    pub fn set_speed(&self, speed: f32) {
        self.0.push(VideoCommand::SetSpeed(speed));
    }

    #[must_use]
    pub fn state(&self) -> VideoState {
        VideoState::from_code(self.0.state.load(Ordering::Relaxed))
    }

    #[must_use]
    pub fn position(&self) -> Duration {
        Duration::from_micros(self.0.position_us.load(Ordering::Relaxed).max(0) as u64)
    }

    #[must_use]
    pub fn duration(&self) -> Option<Duration> {
        let micros = self.0.duration_us.load(Ordering::Relaxed);
        (micros >= 0).then(|| Duration::from_micros(micros as u64))
    }

    #[must_use]
    pub fn size(&self) -> Option<Size> {
        unpack_size(self.0.size.load(Ordering::Relaxed))
    }

    #[must_use]
    pub fn error(&self) -> Option<String> {
        self.0.lock_error().clone()
    }

    pub fn changes(&self) -> impl Stream<Item = ()> + Send + 'static + use<> {
        self.0.changed.changes()
    }

    pub fn watch(&self, listener: impl Fn(VideoEvent) + Send + Sync + 'static) -> VideoWatch {
        let listener: Arc<Listener> = Arc::new(listener);
        let mut listeners = self.0.listeners.lock();
        listeners.retain(|listener| listener.strong_count() > 0);
        listeners.push(Arc::downgrade(&listener));
        VideoWatch {
            _listener: listener,
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0.path
    }

    #[must_use]
    pub fn id(&self) -> u64 {
        self.0.id
    }
}

impl Shared {
    fn push(&self, command: VideoCommand) {
        self.commands.lock().push(command);
    }

    fn lock_error(&self) -> MutexGuard<'_, Option<String>> {
        self.error.lock()
    }

    fn changed(&self) {
        self.changed.set(());
        self.tell(VideoEvent::Changed);
    }

    fn tell(&self, event: VideoEvent) {
        let listeners: Vec<Arc<Listener>> = {
            let mut listeners = self.listeners.lock();
            listeners.retain(|listener| listener.strong_count() > 0);
            listeners.iter().filter_map(Weak::upgrade).collect()
        };
        for listener in listeners {
            listener(event);
        }
    }
}

impl VideoConsumer for VideoController {
    fn take_commands(&self) -> Vec<VideoCommand> {
        std::mem::take(&mut *self.0.commands.lock())
    }

    fn set_size(&self, size: Size) {
        let packed = pack_size(size);
        if self.0.size.swap(packed, Ordering::Relaxed) != packed {
            self.0.changed();
        }
    }

    fn set_state(&self, state: VideoState) {
        if self.0.state.swap(state.code(), Ordering::Relaxed) != state.code() {
            self.0.changed();
        }
    }

    fn set_position(&self, position: Duration) {
        let micros = position.as_micros().min(i64::MAX as u128) as i64;
        if self.0.position_us.swap(micros, Ordering::Relaxed) != micros {
            self.0.tell(VideoEvent::Progressed);
        }
    }

    fn set_duration(&self, duration: Option<Duration>) {
        let micros = duration.map_or(-1, |duration| {
            duration.as_micros().min(i64::MAX as u128) as i64
        });
        self.0.duration_us.store(micros, Ordering::Relaxed);
    }

    fn fail(&self, error: String) {
        tracing::warn!("failed to play {}: {error}", self.0.path.display());
        *self.0.lock_error() = Some(error);
        self.0.size.store(SIZE_UNKNOWN, Ordering::Relaxed);
        self.set_state(VideoState::Failed);
    }
}

fn pack_size(size: Size) -> u64 {
    (u64::from(size.width.to_bits()) << 32) | u64::from(size.height.to_bits())
}

fn unpack_size(packed: u64) -> Option<Size> {
    (packed != SIZE_UNKNOWN).then(|| {
        Size::new(
            f32::from_bits((packed >> 32) as u32),
            f32::from_bits(packed as u32),
        )
    })
}

impl std::fmt::Debug for VideoController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoController")
            .field("id", &self.0.id)
            .field("path", &self.0.path)
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}
