use rikka_sync::Mutex;
use std::sync::Arc;

use futures_channel::mpsc::{UnboundedSender, unbounded};
use futures_util::Stream;
use rikka_worker::Observable;

use crate::event_sink::EventSink;
use crate::refused::Refusing;
use crate::service::{Handler, Service, StartError};

pub trait Apply<E> {
    fn apply(&mut self, event: E) -> bool;
}

impl<T: PartialEq> Apply<T> for T {
    fn apply(&mut self, event: T) -> bool {
        let changed = *self != event;
        if changed {
            *self = event;
        }
        changed
    }
}

pub struct Unavailable(Box<dyn FnOnce(StartError) + Send>);

impl Unavailable {
    pub fn report(self, error: StartError) {
        (self.0)(error);
    }
}

impl std::fmt::Debug for Unavailable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Unavailable")
            .finish_non_exhaustive()
    }
}

pub trait Backend: Sized + 'static {
    type Request: Send + 'static;
    type Event: Clone + Send + 'static;
    type State: Apply<Self::Event> + Clone + Default + std::fmt::Debug + Send + Sync + 'static;

    const UNAVAILABLE: &'static str;

    fn start(events: EventSink<Self::Event>, unavailable: Unavailable) -> Result<Self, StartError>;

    fn request(&self, request: Self::Request);
}

impl<H: Handler> Backend for Service<H>
where
    H::Config: Default,
{
    type Request = H::Request;
    type Event = H::Event;
    type State = H::State;

    const UNAVAILABLE: &'static str = H::UNAVAILABLE;

    fn start(
        events: EventSink<H::Event>,
        unavailable: Unavailable,
    ) -> Result<Service<H>, StartError> {
        Service::start(H::Config::default(), events, |error| {
            unavailable.report(error);
        })
    }

    fn request(&self, request: H::Request) {
        self.send(request);
    }
}

pub struct Monitor<B: Backend>(Arc<Shared<B>>);

struct Shared<B: Backend> {
    backend: Option<B>,
    state: Observable<B::State>,
    unavailable: Observable<Option<String>>,
    listeners: Arc<Listeners<B::Event>>,
}

impl<B: Backend> Monitor<B> {
    #[must_use]
    pub fn start() -> Monitor<B> {
        Monitor::start_with(B::start)
    }

    #[must_use]
    pub fn start_with(
        start: impl FnOnce(EventSink<B::Event>, Unavailable) -> Result<B, StartError>,
    ) -> Monitor<B> {
        let state = Observable::new(B::State::default());
        let unavailable = Observable::new(None);
        let listeners = Arc::new(Listeners::default());
        let target = state.clone();
        let heard = Arc::clone(&listeners);
        let events = EventSink::new(move |event: B::Event| {
            heard.tell(&event);
            target.modify(|state| state.apply(event));
        });
        let failing = unavailable.clone();
        let failed = move |error: StartError| {
            tracing::debug!(%error, "{}", B::UNAVAILABLE);
            failing.set(Some(error.to_string()));
        };
        let backend = start(events, Unavailable(Box::new(failed.clone())))
            .map_err(failed)
            .ok();
        Monitor(Arc::new(Shared {
            backend,
            state,
            unavailable,
            listeners,
        }))
    }

    #[must_use]
    pub fn state(&self) -> B::State {
        self.0.state.get()
    }

    pub fn read<R>(&self, read: impl FnOnce(&B::State) -> R) -> R {
        self.0.state.with(read)
    }

    pub fn changes(&self) -> impl Stream<Item = B::State> + Send + 'static {
        self.0.state.changes()
    }

    pub fn events(&self) -> impl Stream<Item = B::Event> + Send + 'static {
        self.0.listeners.join()
    }

    #[must_use]
    pub fn unavailable(&self) -> Option<String> {
        self.0.unavailable.get()
    }

    pub fn unavailable_changes(&self) -> impl Stream<Item = Option<String>> + Send + 'static {
        self.0.unavailable.changes()
    }

    #[must_use]
    pub fn backend(&self) -> Option<&B> {
        self.0.backend.as_ref()
    }

    pub fn edit(&self, edit: impl FnOnce(&mut B::State) -> bool) {
        self.0.state.modify(edit);
    }

    pub fn request(&self, request: B::Request) {
        if let Some(backend) = &self.0.backend {
            backend.request(request);
        }
    }
}

impl<B: Backend> Monitor<B>
where
    B::State: Refusing,
{
    pub fn attempt(&self, request: B::Request) {
        self.edit(|state| state.refused().take().is_some());
        self.request(request);
    }
}

impl<B: Backend> Clone for Monitor<B> {
    fn clone(&self) -> Monitor<B> {
        Monitor(Arc::clone(&self.0))
    }
}

impl<B: Backend> std::fmt::Debug for Monitor<B> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("Monitor")
            .field(&self.state())
            .finish()
    }
}

struct Listeners<E>(Mutex<Vec<UnboundedSender<E>>>);

impl<E> Default for Listeners<E> {
    fn default() -> Listeners<E> {
        Listeners(Mutex::new(Vec::new()))
    }
}

impl<E: Clone> Listeners<E> {
    fn join(&self) -> impl Stream<Item = E> + Send + 'static
    where
        E: Send + 'static,
    {
        let (sender, events) = unbounded();
        self.0.lock().push(sender);
        events
    }

    fn tell(&self, event: &E) {
        self.0
            .lock()
            .retain(|listener| listener.unbounded_send(event.clone()).is_ok());
    }
}
