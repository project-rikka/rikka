use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use tokio::sync::oneshot;

use crate::call::{Call, Responder};
use crate::capability::CapabilityId;
use crate::dispatcher::Dispatcher;
use crate::error::SeamError;
use crate::event::{EventBus, EventQueue, EventStream};
use crate::payload::Payload;
use crate::request::Request;

#[derive(Clone)]
pub struct SeamClient {
    dispatcher: Arc<dyn Dispatcher>,
    bus: EventBus,
}

impl SeamClient {
    pub(crate) fn new(dispatcher: Arc<dyn Dispatcher>, bus: EventBus) -> Self {
        Self { dispatcher, bus }
    }

    pub fn call(&self, mut request: Request) -> Reply {
        let (sender, receiver) = oneshot::channel();
        let responder = Responder::new(
            request.capability().clone(),
            request.method().clone(),
            sender,
        );
        match request.take_error() {
            Some(error) => responder.fail(error),
            None => self.dispatcher.enqueue(Call::new(request, responder)),
        }
        Reply(receiver)
    }

    pub fn notify(&self, request: Request) {
        drop(self.call(request));
    }

    pub fn subscribe(&self) -> EventStream {
        self.bus.stream(None)
    }

    pub fn subscribe_to(&self, capability: impl Into<CapabilityId>) -> EventStream {
        self.bus.stream(Some(capability.into()))
    }

    pub fn poll_events(&self, capability: Option<impl Into<CapabilityId>>) -> EventQueue {
        self.bus.queue(capability.map(Into::into))
    }
}

impl std::fmt::Debug for SeamClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SeamClient").finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct Reply(oneshot::Receiver<Result<Payload, SeamError>>);

impl Reply {
    pub fn blocking_wait(self) -> Result<Payload, SeamError> {
        rikka_worker::block_on(self)
    }
}

impl Future for Reply {
    type Output = Result<Payload, SeamError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.get_mut().0)
            .poll(cx)
            .map(|received| received.unwrap_or(Err(SeamError::Disconnected)))
    }
}
