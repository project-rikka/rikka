use std::future::Future;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, OnceLock, Weak};
use std::time::Duration;

use discord_core::{Feed, LinkState, MessageId, Update, Updates};
use futures_channel::oneshot;
use rikka_sync::Mutex;
use serde_json::Value;
use tokio::runtime::Handle;

use discord_state::{Moment, Outcome, SUPPLEMENT_WAIT, State};

use crate::error::{Error, Result};
use crate::gateway::{First, Gateway, Listener};
use crate::identity::Identity;
use crate::mint::Mint;
use crate::rest::Rest;
use crate::runtime;
use crate::stream::Streams;
use crate::voice::Voice;

const HOUSEKEEPING: Duration = Duration::from_secs(30);

pub(crate) struct Session {
    this: Weak<Session>,
    runtime: Handle,
    rest: Rest,
    identity: Arc<Identity>,
    state: Mutex<State>,
    premium: AtomicU8,
    mint: Mint,
    feed: Feed,
    gateway: OnceLock<Gateway>,
    voice: Voice,
    streams: Streams,
}

impl Session {
    pub(crate) fn start(
        runtime: Handle,
        rest: Rest,
    ) -> (Arc<Session>, Updates, oneshot::Receiver<First>) {
        let (session, updates) = Session::offline(runtime.clone(), rest);
        let listener: Weak<Session> = Arc::downgrade(&session);
        let listener: Weak<dyn Listener> = listener;
        let (gateway, first) = Gateway::start(
            &runtime,
            Arc::clone(session.rest.token()),
            Arc::clone(&session.identity),
            listener,
        );
        let _ = session.gateway.set(gateway);
        session.start_housekeeping();
        (session, updates, first)
    }

    pub(crate) fn offline(runtime: Handle, rest: Rest) -> (Arc<Session>, Updates) {
        let (feed, updates) = Feed::new();
        let identity = Arc::clone(rest.identity());
        let session = Arc::new_cyclic(|this| Session {
            this: this.clone(),
            runtime,
            rest,
            identity,
            state: Mutex::new(State::new(Moment::now())),
            premium: AtomicU8::new(0),
            mint: Mint::default(),
            feed,
            gateway: OnceLock::new(),
            voice: Voice::default(),
            streams: Streams::default(),
        });
        (session, updates)
    }

    pub(crate) fn rest(&self) -> &Rest {
        &self.rest
    }

    pub(crate) fn runtime(&self) -> &Handle {
        &self.runtime
    }

    pub(crate) fn voice(&self) -> &Voice {
        &self.voice
    }

    pub(crate) fn streams(&self) -> &Streams {
        &self.streams
    }

    pub(crate) fn state(&self) -> rikka_sync::MutexGuard<'_, State> {
        self.state.lock()
    }

    pub(crate) fn premium(&self) -> u8 {
        self.premium.load(Ordering::Relaxed)
    }

    pub(crate) fn mint(&self) -> MessageId {
        self.mint.next(Moment::now().unix_ms)
    }

    pub(crate) fn in_background(
        &self,
        change: impl FnOnce(&mut State) -> Outcome + Send + 'static,
    ) {
        let this = self.this.clone();
        self.runtime.spawn(async move {
            if let Some(session) = this.upgrade() {
                let outcome = change(&mut session.state.lock());
                session.apply(outcome);
            }
        });
    }

    pub(crate) fn send(&self, payload: Value) {
        if let Some(gateway) = self.gateway.get() {
            gateway.send(payload);
        }
    }

    pub(crate) fn emit(&self, update: Update) {
        self.feed.send(update);
    }

    pub(crate) fn apply(&self, outcome: Outcome) {
        for update in outcome.updates {
            self.feed.send(update);
        }
        for command in outcome.commands {
            self.send(command);
        }
        for signal in outcome.voice {
            self.voice.signal(signal);
        }
        for signal in outcome.streams {
            self.streams.signal(signal);
        }
    }

    pub(crate) fn close(&self) {
        self.voice.hang_up(self);
        if let Some(gateway) = self.gateway.get() {
            gateway.close();
        }
        self.feed.send(Update::Link(LinkState::SignedOut));
    }

    pub(crate) async fn work<T, F, Fut>(&self, task: F) -> Result<T>
    where
        F: FnOnce(Arc<Session>) -> Fut,
        Fut: Future<Output = Result<T>> + Send + 'static,
        T: Send + 'static,
    {
        let session = self.this.upgrade().ok_or(Error::Cancelled)?;
        runtime::run(&self.runtime, task(session)).await
    }

    fn start_housekeeping(&self) {
        let this = self.this.clone();
        self.runtime.spawn(async move {
            let mut every = tokio::time::interval(HOUSEKEEPING);
            every.tick().await;
            loop {
                every.tick().await;
                let Some(session) = this.upgrade() else {
                    return;
                };
                if session.feed.is_closed() {
                    if let Some(gateway) = session.gateway.get() {
                        gateway.close();
                    }
                    return;
                }
                let outcome = session.state.lock().tick(Moment::now());
                session.apply(outcome);
            }
        });
    }

    fn supplement_late(&self) {
        let this = self.this.clone();
        self.runtime.spawn(async move {
            tokio::time::sleep(SUPPLEMENT_WAIT).await;
            if let Some(session) = this.upgrade() {
                let outcome = session.state.lock().tick(Moment::now());
                session.apply(outcome);
            }
        });
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Session")
            .field("me", &self.state.lock().me())
            .finish_non_exhaustive()
    }
}

impl Listener for Session {
    fn dispatch(&self, event: &str, data: &str) {
        let outcome = {
            let mut state = self.state.lock();
            let outcome = state.dispatch(event, data, Moment::now());
            if let Some(locale) = state.locale() {
                self.identity.set_locale(locale);
            }
            self.premium.store(state.premium(), Ordering::Relaxed);
            outcome
        };
        self.apply(outcome);
        if event == "READY" {
            self.supplement_late();
        }
    }

    fn link(&self, state: LinkState) {
        self.feed.send(Update::Link(state));
    }
}
