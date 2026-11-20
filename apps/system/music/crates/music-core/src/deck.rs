use rikka_sync::{Mutex, MutexGuard};
use std::ops::ControlFlow;
use std::sync::{Arc, Weak};
use std::thread;
use std::time::Duration;

use rikka_hifi::{Event, Failure, Hifi, SignalPath, Source, SourceFormat, SourceId};
use rikka_worker::{Broadcast, Subscription};

use crate::blocking::drain;
use crate::event::PlaybackEvent;
use crate::fidelity::Fidelity;
use crate::item::Id;

const THREAD: &str = "rikka-music-deck";

#[derive(Clone)]
pub struct Deck {
    shared: Arc<Shared>,
}

struct Shared {
    hifi: Hifi,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    generation: u64,
    listeners: Option<Arc<Broadcast<PlaybackEvent>>>,
    current: Option<Owned>,
    queued: Vec<Owned>,
    fidelity: Option<Fidelity>,
    at: Duration,
}

struct Owned {
    id: SourceId,
    track: Id,
    from: Duration,
    format: SourceFormat,
    started: bool,
}

#[derive(Clone)]
pub struct Lease {
    deck: Deck,
    generation: u64,
}

impl Deck {
    #[must_use]
    pub fn new(hifi: Hifi) -> Deck {
        let events = hifi.events();
        let shared = Arc::new(Shared {
            hifi,
            state: Mutex::default(),
        });
        let relayed = Arc::downgrade(&shared);
        thread::Builder::new()
            .name(THREAD.into())
            .spawn(move || relay(events, &relayed))
            .expect("failed to spawn rikka-music-deck thread");
        Deck { shared }
    }

    #[must_use]
    pub fn hifi(&self) -> &Hifi {
        &self.shared.hifi
    }

    pub fn acquire(&self, listeners: &Arc<Broadcast<PlaybackEvent>>) -> Lease {
        let mut state = self.state();
        state.generation += 1;
        state.listeners = Some(Arc::clone(listeners));
        if state.current.take().is_some() {
            self.shared.hifi.pause();
        }
        state.queued.clear();
        state.fidelity = None;
        Lease {
            deck: self.clone(),
            generation: state.generation,
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.shared.state()
    }
}

impl std::fmt::Debug for Deck {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Deck").finish_non_exhaustive()
    }
}

impl Lease {
    #[must_use]
    pub fn is_current(&self) -> bool {
        self.deck.state().generation == self.generation
    }

    #[must_use]
    pub fn owns(&self, source: SourceId) -> bool {
        let state = self.deck.state();
        self.is_active(&state)
            && state
                .current
                .as_ref()
                .is_some_and(|current| current.id == source)
    }

    pub fn play(&self, track: &Id, source: Source, from: Duration) -> Option<SourceId> {
        let mut state = self.deck.state();
        if state.generation != self.generation {
            return None;
        }
        let format = *source.format();
        let id = self.deck.shared.hifi.play(source);
        state.current = Some(Owned::new(id, track.clone(), from, format));
        state.queued.clear();
        state.fidelity = None;
        state.at = from;
        Some(id)
    }

    pub fn queue(&self, track: &Id, source: Source) -> Option<SourceId> {
        let mut state = self.deck.state();
        if !self.is_active(&state) {
            return None;
        }
        let format = *source.format();
        let id = self.deck.shared.hifi.queue(source);
        state
            .queued
            .push(Owned::new(id, track.clone(), Duration::ZERO, format));
        Some(id)
    }

    pub fn unqueue(&self) {
        if self.is_active(&self.deck.state()) {
            self.deck.shared.hifi.unqueue();
        }
    }
    pub fn pause(&self) {
        if self.is_active(&self.deck.state()) {
            self.deck.shared.hifi.pause();
        }
    }

    pub fn resume(&self) {
        if self.is_active(&self.deck.state()) {
            self.deck.shared.hifi.resume();
        }
    }

    pub fn seek(&self, at: Duration) {
        let mut state = self.deck.state();
        if self.is_active(&state) {
            self.deck.shared.hifi.seek(at);
            state.at = at;
        }
    }

    fn is_active(&self, state: &State) -> bool {
        state.generation == self.generation && state.current.is_some()
    }
}

impl std::fmt::Debug for Lease {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Lease")
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock()
    }
}

impl Owned {
    fn new(id: SourceId, track: Id, from: Duration, format: SourceFormat) -> Owned {
        Owned {
            id,
            track,
            from,
            format,
            started: false,
        }
    }
}

impl State {
    fn handle(&mut self, event: Event) -> Vec<PlaybackEvent> {
        match event {
            Event::Started(id) => self.started(id),
            Event::Advanced { from, to } => self.advanced(from, to),
            Event::Position { source, at } => match self.track_of(source) {
                Some(track) => {
                    self.at = at;
                    vec![PlaybackEvent::Position { track, at }]
                }
                None => Vec::new(),
            },
            Event::Ended(id) => self.ended(id),
            Event::Paused => self.current_event(|track, at| PlaybackEvent::Paused { track, at }),
            Event::Resumed => self.current_event(|track, at| PlaybackEvent::Playing { track, at }),
            Event::Path(path) => self.path(&path),
            Event::Failed { source, failure } => self.failed(source, &failure),
            Event::Stopped | Event::Output(_) => Vec::new(),
        }
    }

    fn started(&mut self, id: SourceId) -> Vec<PlaybackEvent> {
        let Some(current) = self.current.as_mut().filter(|current| current.id == id) else {
            return Vec::new();
        };
        if current.started {
            return Vec::new();
        }
        current.started = true;
        let (track, from, format) = (current.track.clone(), current.from, current.format);
        self.at = from;
        let mut events = vec![PlaybackEvent::Playing {
            track: track.clone(),
            at: from,
        }];
        events.extend(self.fidelity_change(&track, &format));
        events
    }

    fn advanced(&mut self, from: SourceId, to: SourceId) -> Vec<PlaybackEvent> {
        let Some(index) = self.queued.iter().position(|queued| queued.id == to) else {
            return self.started(to);
        };
        let mut next = self.queued.remove(index);
        self.queued.drain(..index);
        next.started = true;
        let (track, format) = (next.track.clone(), next.format);
        let left = self
            .current
            .replace(next)
            .filter(|previous| previous.id == from)
            .map(|previous| previous.track);
        self.at = Duration::ZERO;
        self.fidelity = None;
        let mut events = Vec::new();
        if let Some(left) = left {
            events.push(PlaybackEvent::Advanced {
                from: left,
                to: track.clone(),
            });
        }
        events.push(PlaybackEvent::Playing {
            track: track.clone(),
            at: Duration::ZERO,
        });
        events.extend(self.fidelity_change(&track, &format));
        events
    }

    fn ended(&mut self, id: SourceId) -> Vec<PlaybackEvent> {
        let Some(ended) = self.current.take_if(|current| current.id == id) else {
            return Vec::new();
        };
        self.queued.clear();
        self.fidelity = None;
        vec![PlaybackEvent::Ended { track: ended.track }]
    }

    fn path(&mut self, path: &SignalPath) -> Vec<PlaybackEvent> {
        let Some(track) = self
            .current
            .as_ref()
            .filter(|current| current.started)
            .map(|current| current.track.clone())
        else {
            return Vec::new();
        };
        self.fidelity_change(&track, &path.source)
            .into_iter()
            .collect()
    }

    fn failed(&mut self, source: Option<SourceId>, failure: &Failure) -> Vec<PlaybackEvent> {
        let message = failure.to_string();
        let current = self.current.as_ref().map(|current| current.id);
        match source {
            Some(id) if Some(id) == current => {
                let unstarted = self
                    .current
                    .as_ref()
                    .is_some_and(|current| !current.started);
                if !unstarted {
                    return vec![PlaybackEvent::Failed(message)];
                }
                self.current
                    .take()
                    .map(|current| PlaybackEvent::Unavailable {
                        track: current.track,
                    })
                    .into_iter()
                    .collect()
            }
            Some(id) => {
                self.queued.retain(|queued| queued.id != id);
                Vec::new()
            }
            None if current.is_some() => {
                let mut events = vec![PlaybackEvent::Failed(message)];
                events.extend(self.current_event(|track, at| PlaybackEvent::Paused { track, at }));
                events
            }
            None => Vec::new(),
        }
    }

    fn fidelity_change(&mut self, track: &Id, format: &SourceFormat) -> Option<PlaybackEvent> {
        let fidelity = Fidelity::from_source_format(format);
        if self.fidelity == Some(fidelity) {
            return None;
        }
        self.fidelity = Some(fidelity);
        Some(PlaybackEvent::Sounding {
            track: track.clone(),
            fidelity,
        })
    }

    fn current_event(
        &self,
        events: impl FnOnce(Id, Duration) -> PlaybackEvent,
    ) -> Vec<PlaybackEvent> {
        self.current
            .as_ref()
            .map(|current| events(current.track.clone(), self.at))
            .into_iter()
            .collect()
    }

    fn track_of(&self, source: SourceId) -> Option<Id> {
        self.current
            .as_ref()
            .filter(|current| current.id == source)
            .map(|current| current.track.clone())
    }
}

fn relay(events: Subscription<Event>, shared: &Weak<Shared>) {
    drain(events, |event| {
        let Some(shared) = shared.upgrade() else {
            return ControlFlow::Break(());
        };
        let (listeners, published) = {
            let mut state = shared.state();
            let published = state.handle(event);
            (state.listeners.clone(), published)
        };
        if let Some(listeners) = listeners {
            for event in &published {
                listeners.send(event);
            }
        }
        ControlFlow::Continue(())
    });
}
