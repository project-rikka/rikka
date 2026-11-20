use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use music_core::{
    Deck, Error, Events, Id, Lease, Playback, PlaybackEvent, ProviderCache, Quality, Result,
};
use rikka_hifi::Source;
use rikka_worker::Broadcast;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::task::AbortHandle;

use crate::opening::{Opening, Ready};
use crate::origin::Origin;
use crate::spool::{Spool, Stored};

enum Command {
    Load {
        track: Id,
        at: Duration,
        play: bool,
        lease: Lease,
    },
    Prepare(Option<Id>),
    Play,
    Pause,
    Seek(Duration),
    Quality(Quality),
}

const THREAD: &str = "rikka-music-play";

pub struct Streamer {
    commands: UnboundedSender<Command>,
    listeners: Arc<Broadcast<PlaybackEvent>>,
    deck: Deck,
}

impl Streamer {
    pub fn start(origin: Arc<dyn Origin>, cache: ProviderCache, deck: Deck) -> Result<Streamer> {
        let (commands, queue) = unbounded_channel();
        let listeners = Arc::new(Broadcast::new());
        let published = Arc::clone(&listeners);
        let events = listeners.subscribe();
        std::thread::Builder::new()
            .name(THREAD.into())
            .spawn(move || serve(origin, cache, published, queue, events))
            .map_err(|error| Error::Spawn(format!("{THREAD}: {error}")))?;
        Ok(Streamer {
            commands,
            listeners,
            deck,
        })
    }

    fn send(&self, command: Command) {
        if self.commands.send(command).is_err() {
            self.listeners
                .send(&PlaybackEvent::Failed("the player has stopped".into()));
        }
    }
}

impl std::fmt::Debug for Streamer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Streamer").finish_non_exhaustive()
    }
}

impl Playback for Streamer {
    fn load(&self, track: &Id, at: Duration, play: bool) {
        let lease = self.deck.acquire(&self.listeners);
        self.send(Command::Load {
            track: track.clone(),
            at,
            play,
            lease,
        });
    }

    fn prepare(&self, next: Option<&Id>) {
        self.send(Command::Prepare(next.cloned()));
    }

    fn play(&self) {
        self.send(Command::Play);
    }

    fn pause(&self) {
        self.send(Command::Pause);
    }

    fn seek(&self, at: Duration) {
        self.send(Command::Seek(at));
    }

    fn set_quality(&self, quality: Quality) {
        self.send(Command::Quality(quality));
    }

    fn events(&self) -> Events {
        self.listeners.subscribe()
    }
}

enum Arrival {
    Fetched {
        epoch: u64,
        key: String,
        staged: PathBuf,
        outcome: Result<()>,
    },
    Opened {
        epoch: u64,
        ready: std::result::Result<Box<Ready>, String>,
    },
}

struct Download {
    fetching: AbortHandle,
    staged: PathBuf,
}

impl Drop for Download {
    fn drop(&mut self) {
        self.fetching.abort();
        let _ = std::fs::remove_file(&self.staged);
    }
}

enum Stage {
    Waiting,
    Opening { at: Duration, play: bool },
    Parked { source: Source, at: Duration },
    Ready(Source),
    Handed,
}

struct Slot {
    track: Id,
    epoch: u64,
    download: Option<Download>,
    cached: Option<PathBuf>,
    stage: Stage,
}

struct Engine {
    origin: Arc<dyn Origin>,
    cache: ProviderCache,
    listeners: Arc<Broadcast<PlaybackEvent>>,
    arrivals: UnboundedSender<Arrival>,
    lease: Option<Lease>,
    current: Option<Slot>,
    next: Option<Slot>,
    following: Option<Id>,
    spools: Option<PathBuf>,
    epoch: u64,
    quality: Quality,
}

fn serve(
    origin: Arc<dyn Origin>,
    cache: ProviderCache,
    listeners: Arc<Broadcast<PlaybackEvent>>,
    queue: UnboundedReceiver<Command>,
    events: Events,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            listeners.send(&PlaybackEvent::Failed(error.to_string()));
            return;
        }
    };
    let (arrivals, arriving) = unbounded_channel();
    let mut engine = Engine {
        origin,
        cache,
        listeners,
        arrivals,
        lease: None,
        current: None,
        next: None,
        following: None,
        spools: None,
        epoch: 0,
        quality: Quality::default(),
    };
    runtime.block_on(engine.run(queue, arriving, events));
    engine.current = None;
    engine.next = None;
    drop(runtime);
    if let Some(spools) = engine.spools.take() {
        let _ = std::fs::remove_dir_all(spools);
    }
}

impl Engine {
    async fn run(
        &mut self,
        mut queue: UnboundedReceiver<Command>,
        mut arriving: UnboundedReceiver<Arrival>,
        mut events: Events,
    ) {
        loop {
            tokio::select! {
                biased;
                Some(event) = events.next() => self.on_event(&event),
                Some(arrival) = arriving.recv() => self.arrive(arrival),
                command = queue.recv() => match command {
                    Some(command) => self.handle_command(command),
                    None => break,
                },
            }
        }
    }

    fn handle_command(&mut self, command: Command) {
        match command {
            Command::Load {
                track,
                at,
                play,
                lease,
            } => self.load(track, at, play, lease),
            Command::Prepare(next) => self.prepare(next),
            Command::Play => self.play(),
            Command::Pause => self.pause(),
            Command::Seek(at) => self.seek(at),
            Command::Quality(quality) => self.quality = quality,
        }
    }

    fn load(&mut self, track: Id, at: Duration, play: bool, lease: Lease) {
        self.lease = Some(lease);
        self.next = None;
        self.following = None;
        self.current = None;
        self.listeners.send(&PlaybackEvent::Loading {
            track: track.clone(),
        });
        match self.slot(track.clone(), Stage::Opening { at, play }) {
            Ok(slot) => self.current = Some(slot),
            Err(error) => {
                tracing::warn!("no source for {track}: {error}");
                self.listeners.send(&PlaybackEvent::Unavailable { track });
            }
        }
    }

    fn prepare(&mut self, next: Option<Id>) {
        if self.following == next {
            return;
        }
        self.following.clone_from(&next);
        if self
            .next
            .take()
            .is_some_and(|slot| matches!(slot.stage, Stage::Handed))
            && let Some(lease) = &self.lease
        {
            lease.unqueue();
        }
        let Some(track) = next else {
            return;
        };
        let downloading = self
            .current
            .as_ref()
            .is_some_and(|current| current.download.is_some());
        if downloading {
            self.epoch += 1;
            self.next = Some(Slot {
                track,
                epoch: self.epoch,
                download: None,
                cached: None,
                stage: Stage::Waiting,
            });
            return;
        }
        self.open_next(track);
    }

    fn open_next(&mut self, track: Id) {
        let opening = Stage::Opening {
            at: Duration::ZERO,
            play: true,
        };
        match self.slot(track.clone(), opening) {
            Ok(slot) => self.next = Some(slot),
            Err(error) => tracing::debug!("failed to prepare {track}: {error}"),
        }
    }

    fn play(&mut self) {
        let Some(current) = self.current.as_mut() else {
            return;
        };
        match std::mem::replace(&mut current.stage, Stage::Handed) {
            Stage::Parked { source, at } => self.hand(source, at),
            Stage::Opening { at, .. } => current.stage = Stage::Opening { at, play: true },
            Stage::Handed => {
                if let Some(lease) = &self.lease {
                    lease.resume();
                }
            }
            other => current.stage = other,
        }
    }

    fn pause(&mut self) {
        let Some(current) = self.current.as_mut() else {
            return;
        };
        match &mut current.stage {
            Stage::Opening { play, .. } => *play = false,
            Stage::Handed => {
                if let Some(lease) = &self.lease {
                    lease.pause();
                }
            }
            Stage::Waiting | Stage::Parked { .. } | Stage::Ready(_) => {}
        }
    }

    fn seek(&mut self, to: Duration) {
        let Some(current) = self.current.as_mut() else {
            return;
        };
        match &mut current.stage {
            Stage::Opening { at, .. } | Stage::Parked { at, .. } => *at = to,
            Stage::Handed => {
                if let Some(lease) = &self.lease {
                    lease.seek(to);
                }
                self.listeners.send(&PlaybackEvent::Position {
                    track: current.track.clone(),
                    at: to,
                });
            }
            Stage::Waiting | Stage::Ready(_) => {}
        }
    }

    fn on_event(&mut self, event: &PlaybackEvent) {
        let PlaybackEvent::Advanced { to, .. } = event else {
            return;
        };
        let advanced = self
            .next
            .as_ref()
            .is_some_and(|next| next.track == *to && matches!(next.stage, Stage::Handed));
        if advanced {
            self.current = self.next.take();
            self.following = None;
        }
    }

    fn slot(&mut self, track: Id, stage: Stage) -> Result<Slot> {
        self.epoch += 1;
        let epoch = self.epoch;
        let at = match stage {
            Stage::Opening { at, .. } => at,
            _ => Duration::ZERO,
        };
        let (opening, download, cached) = self.source(&track, epoch)?;
        let origin = Arc::clone(&self.origin);
        let arrivals = self.arrivals.clone();
        let requested = track.clone();
        tokio::spawn(async move {
            let streamed = opening.is_streamed();
            let reading = Arc::clone(&origin);
            let ready = tokio::task::spawn_blocking(move || opening.ready(reading.as_ref(), at));
            let gains = async {
                match streamed {
                    true => Some(origin.gains(&requested).await),
                    false => None,
                }
            };
            let (ready, gains) = tokio::join!(ready, gains);
            let ready =
                ready
                    .unwrap_or_else(|error| Err(error.to_string()))
                    .map(|ready| match gains {
                        Some(gains) => Box::new(ready.gained(gains)),
                        None => Box::new(ready),
                    });
            let _ = arrivals.send(Arrival::Opened { epoch, ready });
        });
        Ok(Slot {
            track,
            epoch,
            download,
            cached,
            stage,
        })
    }

    fn source(
        &mut self,
        track: &Id,
        epoch: u64,
    ) -> Result<(Opening, Option<Download>, Option<PathBuf>)> {
        if let Some(located) = self.origin.locate(track) {
            return Ok((Opening::Located(located), None, None));
        }
        let key = format!("{track}.{}", self.quality.slug());
        if let Some(cached) = self.cache.find(&key) {
            match Stored::whole(&cached) {
                Ok(stored) => return Ok((Opening::Stored(stored), None, Some(cached))),
                Err(gone) => tracing::debug!("cached {key} missing, fetching: {gone}"),
            }
        }
        let folder = match self.spools.clone() {
            Some(folder) => folder,
            None => {
                let folder = self.cache.spool()?;
                self.spools = Some(folder.clone());
                folder
            }
        };
        let mut spool = Spool::create(&folder.join(format!("{epoch}.audio")))?;
        let stored = spool.stored()?;
        let staged = spool.path().to_owned();
        let origin = Arc::clone(&self.origin);
        let arrivals = self.arrivals.clone();
        let quality = self.quality;
        let track = track.clone();
        let landing = staged.clone();
        let fetching = tokio::spawn(async move {
            let outcome = match origin.fetch(&track, quality, &mut spool).await {
                Ok(()) => spool.finish(),
                Err(error) => Err(error),
            };
            let failure = outcome.as_ref().err().map(ToString::to_string);
            let _ = arrivals.send(Arrival::Fetched {
                epoch,
                key,
                staged: landing,
                outcome,
            });
            if let Some(reason) = failure {
                spool.fail(&reason);
            }
        });
        let download = Download {
            fetching: fetching.abort_handle(),
            staged,
        };
        Ok((Opening::Stored(stored), Some(download), None))
    }

    fn arrive(&mut self, arrival: Arrival) {
        match arrival {
            Arrival::Fetched {
                epoch,
                key,
                staged,
                outcome,
            } => self.fetched(epoch, &key, &staged, outcome),
            Arrival::Opened { epoch, ready } => {
                if self
                    .current
                    .as_ref()
                    .is_some_and(|slot| slot.epoch == epoch)
                {
                    self.current_opened(ready);
                } else if self.next.as_ref().is_some_and(|slot| slot.epoch == epoch) {
                    self.next_opened(ready);
                }
            }
        }
    }

    fn fetched(&mut self, epoch: u64, key: &str, staged: &Path, outcome: Result<()>) {
        let refused = matches!(outcome, Err(Error::Refused(_)));
        let stored = outcome.and_then(|()| self.cache.insert(key, staged));
        if let Err(error) = stored {
            tracing::warn!("failed to store {key}: {error}");
            let _ = std::fs::remove_file(staged);
        }
        if refused
            && let Some(current) = self.current.take_if(|current| {
                current.epoch == epoch && matches!(current.stage, Stage::Opening { .. })
            })
        {
            self.next = None;
            self.following = None;
            self.listeners.send(&PlaybackEvent::Refused {
                track: current.track,
            });
        }
        for slot in [&mut self.current, &mut self.next].into_iter().flatten() {
            if slot.epoch == epoch {
                slot.download = None;
            }
        }
        let waiting = self
            .next
            .as_ref()
            .filter(|next| matches!(next.stage, Stage::Waiting))
            .map(|next| next.track.clone());
        let idle = self
            .current
            .as_ref()
            .is_none_or(|current| current.download.is_none());
        if let Some(track) = waiting
            && idle
        {
            self.next = None;
            self.open_next(track);
        }
    }

    fn current_opened(&mut self, ready: std::result::Result<Box<Ready>, String>) {
        let Some(current) = self.current.as_mut() else {
            return;
        };
        let track = current.track.clone();
        let ready = match ready {
            Ok(ready) => ready,
            Err(reason) => {
                let (at, play) = match current.stage {
                    Stage::Opening { at, play } => (at, play),
                    _ => (Duration::ZERO, true),
                };
                match (current.cached.take(), self.lease.clone()) {
                    (Some(cached), Some(lease)) => {
                        tracing::warn!("cached {track} unreadable, refetching: {reason}");
                        let _ = std::fs::remove_file(cached);
                        self.load(track, at, play, lease);
                    }
                    _ => {
                        tracing::warn!("cannot play {track}: {reason}");
                        self.current = None;
                        self.listeners.send(&PlaybackEvent::Unavailable { track });
                    }
                }
                return;
            }
        };
        if let Some(length) = ready.length {
            self.listeners.send(&PlaybackEvent::Length {
                track: track.clone(),
                length,
            });
        }
        self.listeners.send(&PlaybackEvent::Sounding {
            track: track.clone(),
            fidelity: ready.fidelity,
        });
        match std::mem::replace(&mut current.stage, Stage::Handed) {
            Stage::Opening { at, play: true } => self.hand(ready.source, at),
            Stage::Opening { at, play: false } => {
                current.stage = Stage::Parked {
                    source: ready.source,
                    at,
                };
                self.listeners.send(&PlaybackEvent::Paused { track, at });
            }
            other => current.stage = other,
        }
    }

    fn next_opened(&mut self, ready: std::result::Result<Box<Ready>, String>) {
        let Some(next) = self.next.as_mut() else {
            return;
        };
        match ready {
            Ok(ready) => {
                next.stage = Stage::Ready(ready.source);
                self.queue_next();
            }
            Err(reason) => {
                tracing::debug!("failed to prepare {}: {reason}", next.track);
                if let Some(cached) = next.cached.take() {
                    let _ = std::fs::remove_file(cached);
                }
                self.next = None;
            }
        }
    }

    fn hand(&mut self, source: Source, at: Duration) {
        let (Some(lease), Some(current)) = (&self.lease, self.current.as_mut()) else {
            return;
        };
        current.stage = Stage::Handed;
        if lease.play(&current.track, source, at).is_some() && !at.is_zero() {
            lease.seek(at);
        }
        self.queue_next();
    }

    fn queue_next(&mut self) {
        let handed = self
            .current
            .as_ref()
            .is_some_and(|current| matches!(current.stage, Stage::Handed));
        let (Some(lease), Some(next)) = (&self.lease, self.next.as_mut()) else {
            return;
        };
        if !handed || !matches!(next.stage, Stage::Ready(_)) {
            return;
        }
        let Stage::Ready(source) = std::mem::replace(&mut next.stage, Stage::Handed) else {
            return;
        };
        if lease.queue(&next.track, source).is_none() {
            self.next = None;
        }
    }
}
