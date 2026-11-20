use rikka_sync::{Mutex, MutexGuard};
use std::collections::{BTreeSet, VecDeque};
use std::pin::Pin;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Weak};
use std::task::{Context, Poll};
use std::time::Duration;

use futures_util::stream::{BoxStream, Stream, StreamExt};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::Notify;

use crate::capability::CapabilityId;
use crate::error::SeamError;
use crate::manifest::Manifest;
use crate::payload::Payload;

const QUEUE_CAPACITY: usize = 256;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeliveryMode {
    #[default]
    Every,
    Latest,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Event {
    capability: CapabilityId,
    name: Box<str>,
    payload: Payload,
    #[serde(default, skip_serializing_if = "DeliveryMode::is_every")]
    delivery: DeliveryMode,
}

impl DeliveryMode {
    fn is_every(&self) -> bool {
        *self == DeliveryMode::Every
    }
}

impl Event {
    pub fn capability(&self) -> &CapabilityId {
        &self.capability
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn payload(&self) -> &Payload {
        &self.payload
    }

    pub fn delivery_mode(&self) -> DeliveryMode {
        self.delivery
    }

    pub fn decode<T: DeserializeOwned>(&self) -> Result<T, SeamError> {
        self.payload.decode()
    }

    fn is_same_event(&self, other: &Event) -> bool {
        self.capability == other.capability && self.name == other.name
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Delivery {
    Event(Event),
    Lagged(Lagged),
}

impl Delivery {
    #[must_use]
    pub fn event(&self) -> Option<&Event> {
        match self {
            Delivery::Event(event) => Some(event),
            Delivery::Lagged(_) => None,
        }
    }

    #[must_use]
    pub fn lagged(&self) -> Option<&Lagged> {
        match self {
            Delivery::Lagged(lagged) => Some(lagged),
            Delivery::Event(_) => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lagged {
    capabilities: Vec<CapabilityId>,
    count: u64,
}

impl Lagged {
    #[must_use]
    pub fn new(capabilities: Vec<CapabilityId>, count: u64) -> Lagged {
        Lagged {
            capabilities,
            count,
        }
    }

    #[must_use]
    pub fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    #[must_use]
    pub fn includes(&self, capability: &CapabilityId) -> bool {
        self.capabilities.contains(capability)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct EventBus(Arc<Mutex<Bus>>);

#[derive(Debug, Default)]
struct Bus {
    closed: bool,
    listening: Vec<Weak<Listening>>,
}

impl Drop for Bus {
    fn drop(&mut self) {
        for listening in self.listening.drain(..) {
            if let Some(listening) = listening.upgrade() {
                listening.close();
            }
        }
    }
}

#[derive(Debug)]
struct Listening {
    filter: Option<CapabilityId>,
    state: Mutex<QueueState>,
    wake: Notify,
}

#[derive(Debug, Default)]
struct QueueState {
    waiting: VecDeque<Event>,
    lost: BTreeSet<CapabilityId>,
    lagged: u64,
    closed: bool,
}

impl EventBus {
    pub(crate) fn new() -> Self {
        Self(Arc::new(Mutex::new(Bus::default())))
    }

    pub(crate) fn send(&self, event: Event) {
        let mut bus = self.bus();
        if bus.closed {
            return;
        }
        bus.listening.retain(|listening| {
            let Some(listening) = listening.upgrade() else {
                return false;
            };
            if matches_filter(&listening.filter, &event) {
                listening.take(event.clone());
            }
            true
        });
    }

    #[cfg(unix)]
    pub(crate) fn close(&self) {
        let mut bus = self.bus();
        bus.closed = true;
        for listening in bus.listening.drain(..) {
            if let Some(listening) = listening.upgrade() {
                listening.close();
            }
        }
    }

    pub(crate) fn stream(&self, filter: Option<CapabilityId>) -> EventStream {
        EventStream::new(self.listening(filter))
    }

    pub(crate) fn queue(&self, filter: Option<CapabilityId>) -> EventQueue {
        EventQueue {
            listening: self.listening(filter),
        }
    }

    pub(crate) fn lagged(&self, lagged: &Lagged) {
        let mut bus = self.bus();
        bus.listening.retain(|listening| {
            let Some(listening) = listening.upgrade() else {
                return false;
            };
            listening.lost(lagged);
            true
        });
    }

    fn listening(&self, filter: Option<CapabilityId>) -> Arc<Listening> {
        let listening = Arc::new(Listening {
            filter,
            state: Mutex::new(QueueState::default()),
            wake: Notify::new(),
        });
        let mut bus = self.bus();
        match bus.closed {
            true => listening.close(),
            false => bus.listening.push(Arc::downgrade(&listening)),
        }
        listening
    }

    fn bus(&self) -> MutexGuard<'_, Bus> {
        self.0.lock()
    }
}

impl Listening {
    fn take(&self, event: Event) {
        let mut state = self.state();
        if event.delivery_mode() == DeliveryMode::Latest
            && let Some(pending) = state
                .waiting
                .iter_mut()
                .find(|waiting| waiting.is_same_event(&event))
        {
            *pending = event;
        } else {
            state.waiting.push_back(event);
        }
        while state.waiting.len() > QUEUE_CAPACITY {
            if let Some(dropped) = state.waiting.pop_front() {
                state.lost.insert(dropped.capability().clone());
                state.lagged += 1;
            }
        }
        drop(state);
        self.wake.notify_waiters();
    }

    fn lost(&self, lagged: &Lagged) {
        let affected: Vec<&CapabilityId> = lagged
            .capabilities()
            .iter()
            .filter(|capability| matches_capability(&self.filter, capability))
            .collect();
        if affected.is_empty() {
            return;
        }
        let mut state = self.state();
        for capability in affected {
            state.lost.insert(capability.clone());
        }
        state.lagged += lagged.count();
        drop(state);
        self.wake.notify_waiters();
    }

    fn close(&self) {
        self.state().closed = true;
        self.wake.notify_waiters();
    }

    fn next_delivery(&self) -> Option<Delivery> {
        let mut state = self.state();
        if state.lagged > 0 {
            let lagged = Lagged {
                capabilities: std::mem::take(&mut state.lost).into_iter().collect(),
                count: std::mem::take(&mut state.lagged),
            };
            return Some(Delivery::Lagged(lagged));
        }
        state.waiting.pop_front().map(Delivery::Event)
    }

    async fn next(&self) -> Option<Delivery> {
        loop {
            let waking = self.wake.notified();
            tokio::pin!(waking);
            waking.as_mut().enable();
            if let Some(delivery) = self.next_delivery() {
                return Some(delivery);
            }
            if self.state().closed {
                return None;
            }
            waking.await;
        }
    }

    fn state(&self) -> MutexGuard<'_, QueueState> {
        self.state.lock()
    }
}

#[derive(Debug)]
pub struct EventQueue {
    listening: Arc<Listening>,
}

impl EventQueue {
    pub fn blocking_next(&mut self) -> Option<Delivery> {
        rikka_worker::block_on(self.listening.next())
    }

    pub fn next_timeout(&mut self, timeout: Duration) -> Result<Delivery, RecvTimeoutError> {
        match rikka_worker::block_on_timeout(self.listening.next(), timeout) {
            Some(Some(delivery)) => Ok(delivery),
            Some(None) => Err(RecvTimeoutError::Disconnected),
            None => Err(RecvTimeoutError::Timeout),
        }
    }

    pub fn try_next(&mut self) -> Option<Delivery> {
        self.listening.next_delivery()
    }
}

fn matches_filter(filter: &Option<CapabilityId>, event: &Event) -> bool {
    matches_capability(filter, event.capability())
}

fn matches_capability(filter: &Option<CapabilityId>, capability: &CapabilityId) -> bool {
    filter.as_ref().is_none_or(|id| capability == id)
}

#[derive(Clone)]
pub struct Publisher {
    manifest: Arc<Manifest>,
    bus: EventBus,
}

impl Publisher {
    pub(crate) fn new(manifest: Arc<Manifest>, bus: EventBus) -> Self {
        Self { manifest, bus }
    }

    pub fn capability(&self) -> &CapabilityId {
        self.manifest.capability()
    }

    pub fn publish(&self, name: &str, payload: impl Into<Payload>) -> Result<(), SeamError> {
        let Some(declared) = self.manifest.declared_event(name) else {
            return Err(SeamError::UnknownEvent {
                capability: self.capability().clone(),
                event: name.to_owned(),
            });
        };
        self.bus.send(Event {
            capability: self.capability().clone(),
            name: name.into(),
            payload: payload.into(),
            delivery: declared.delivery_mode(),
        });
        Ok(())
    }

    pub fn emit<T: Serialize + ?Sized>(&self, name: &str, value: &T) -> Result<(), SeamError> {
        self.publish(name, Payload::encode(value)?)
    }
}

impl std::fmt::Debug for Publisher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Publisher")
            .field("capability", self.capability())
            .finish_non_exhaustive()
    }
}

pub struct EventStream {
    inner: BoxStream<'static, Delivery>,
}

impl EventStream {
    fn new(listening: Arc<Listening>) -> Self {
        let inner = futures_util::stream::unfold(listening, |listening| async move {
            let delivery = listening.next().await?;
            Some((delivery, listening))
        });
        Self {
            inner: inner.boxed(),
        }
    }
}

impl Stream for EventStream {
    type Item = Delivery;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().inner.as_mut().poll_next(cx)
    }
}

impl std::fmt::Debug for EventStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventStream").finish_non_exhaustive()
    }
}
