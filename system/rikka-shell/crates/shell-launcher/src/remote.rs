use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use futures_util::{Stream, StreamExt};
use rikka_sync::{Mutex, MutexGuard};
use rikka_worker::Observable;

use crate::provider::{Activation, Hit, Query, QueryToken, RemoteSearch};

pub(super) struct RemoteProvider {
    asking: Sender<RemoteQuery>,
    turn: Arc<AtomicU64>,
    state: Arc<Mutex<State>>,
    generation: Observable<u64>,
    taken: fn(&Hit) -> Activation,
    answering: bool,
}

struct RemoteQuery {
    turn: u64,
    query: Query,
}

#[derive(Default)]
pub(crate) struct State {
    sent: u64,
    text: String,
    forced: bool,
    answered: u64,
    hits: Vec<Hit>,
}

impl RemoteProvider {
    pub(crate) fn spawn<P: RemoteSearch>(mut provider: P) -> RemoteProvider {
        let (asking, sent) = channel();
        let turn = Arc::new(AtomicU64::new(0));
        let state = Arc::new(Mutex::new(State::default()));
        let generation = Observable::new(0);

        let worker_turn = Arc::clone(&turn);
        let worker_state = Arc::clone(&state);
        let worker_generation = generation.clone();
        let named = provider.info().id().to_owned();
        let answering = std::thread::Builder::new()
            .name(format!("rikka-launcher-{named}"))
            .spawn(move || {
                answer(
                    &mut provider,
                    &sent,
                    &worker_turn,
                    &worker_state,
                    &worker_generation,
                )
            })
            .inspect_err(|error| {
                tracing::warn!("launcher provider {named} failed: {error}");
            })
            .is_ok();

        RemoteProvider {
            asking,
            turn,
            state,
            generation,
            taken: P::taken,
            answering,
        }
    }

    pub(crate) fn ask(&self, query: &Query) -> Vec<Hit> {
        if !self.answering {
            return Vec::new();
        }
        let mut state = self.state();
        if state.text != query.text() || state.forced != query.is_forced() {
            state.sent += 1;
            state.text = query.text().to_owned();
            state.forced = query.is_forced();
            self.turn.store(state.sent, Ordering::Relaxed);
            let request = RemoteQuery {
                turn: state.sent,
                query: query.clone(),
            };
            if self.asking.send(request).is_err() {
                tracing::warn!("remote launcher provider thread stopped");
            }
        }
        match state.answered == state.sent {
            true => state.hits.clone(),
            false => Vec::new(),
        }
    }

    pub(super) fn arrivals(&self) -> impl Stream<Item = ()> + Send + 'static {
        self.generation.changes().map(|_| ())
    }

    pub(crate) fn activate(&self, hit: &Hit) -> Activation {
        (self.taken)(hit)
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock()
    }
}

fn answer<P: RemoteSearch>(
    provider: &mut P,
    sent: &Receiver<RemoteQuery>,
    turn: &Arc<AtomicU64>,
    state: &Arc<Mutex<State>>,
    generation: &Observable<u64>,
) {
    while let Ok(mut asking) = sent.recv() {
        while let Ok(newer) = sent.try_recv() {
            asking = newer;
        }
        let token = QueryToken::new(Arc::clone(turn), asking.turn);
        if token.is_stale() {
            continue;
        }
        let hits = provider.search(&asking.query, &token);
        if token.is_stale() {
            continue;
        }
        let mut answered = state.lock();
        answered.answered = asking.turn;
        answered.hits = hits;
        drop(answered);
        generation.set(asking.turn);
    }
}

impl std::fmt::Debug for RemoteProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.state();
        f.debug_struct("RemoteProvider")
            .field("pending", &(state.answered != state.sent))
            .finish_non_exhaustive()
    }
}
