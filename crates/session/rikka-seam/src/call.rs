use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::oneshot;

use crate::capability::{CapabilityId, Version};
use crate::error::SeamError;
use crate::method::Method;
use crate::payload::Payload;
use crate::request::Request;

#[derive(Debug)]
pub struct Call {
    request: Request,
    responder: Responder,
}

impl Call {
    pub(crate) fn new(request: Request, responder: Responder) -> Self {
        Self { request, responder }
    }

    pub fn capability(&self) -> &CapabilityId {
        self.request.capability()
    }

    pub fn method(&self) -> &Method {
        self.request.method()
    }

    pub fn required_version(&self) -> Option<Version> {
        self.request.required_version()
    }

    pub fn payload(&self) -> &Payload {
        self.request.payload()
    }

    pub fn decode<T: DeserializeOwned>(&self) -> Result<T, SeamError> {
        self.request.decode()
    }

    pub fn encode<T: Serialize + ?Sized>(self, value: &T) {
        self.responder.encode(value);
    }

    pub fn done(self) {
        self.responder.done();
    }

    pub fn fail(self, error: SeamError) {
        self.responder.fail(error);
    }

    pub fn answer(self, result: Result<Payload, SeamError>) {
        self.responder.answer(result);
    }

    pub fn unknown_method(self) {
        let error = SeamError::unknown_method(self.capability(), self.method().as_str());
        self.fail(error);
    }

    pub fn defer(self) -> (Request, Responder) {
        (self.request, self.responder)
    }
}

pub struct Responder {
    capability: CapabilityId,
    method: Method,
    reply: Option<oneshot::Sender<Result<Payload, SeamError>>>,
}

impl Responder {
    pub(crate) fn new(
        capability: CapabilityId,
        method: Method,
        reply: oneshot::Sender<Result<Payload, SeamError>>,
    ) -> Self {
        Self {
            capability,
            method,
            reply: Some(reply),
        }
    }

    pub fn capability(&self) -> &CapabilityId {
        &self.capability
    }

    pub fn method(&self) -> &Method {
        &self.method
    }

    pub fn encode<T: Serialize + ?Sized>(self, value: &T) {
        self.answer(Payload::encode(value));
    }

    pub fn done(self) {
        self.answer(Ok(Payload::none()));
    }

    pub fn fail(self, error: SeamError) {
        self.answer(Err(error));
    }

    pub fn answer(mut self, result: Result<Payload, SeamError>) {
        if let Some(reply) = self.reply.take() {
            let _ = reply.send(result);
        }
    }
}

impl Drop for Responder {
    fn drop(&mut self) {
        if let Some(reply) = self.reply.take() {
            tracing::warn!(
                "`{}/{}` dropped call without reply",
                self.capability,
                self.method
            );
            let _ = reply.send(Err(SeamError::NoReply {
                capability: self.capability.clone(),
                method: self.method.as_str().to_owned(),
            }));
        }
    }
}

impl std::fmt::Debug for Responder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Responder")
            .field("capability", &self.capability)
            .field("method", &self.method)
            .field("answered", &self.reply.is_none())
            .finish_non_exhaustive()
    }
}
