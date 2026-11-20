use rikka_sync::{Mutex, MutexGuard};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::sync::mpsc::Sender;

use rikka_gecko_protocol::geckoview::session::InitData;
use rikka_gecko_protocol::message::{Close, Listen};
use rikka_gecko_protocol::{
    CallbackId, EmbedderMessage, ForGecko, Reply, Request, Target, WindowId,
};
use serde::Serialize;
use serde_json::Value;

use crate::error::Error;
use crate::wire::asked::Asked;
use crate::wire::calls::Calls;
use crate::wire::gates::Gates;

#[derive(Debug)]
pub(crate) struct Link {
    out: Mutex<Out>,
    calls: Calls,
}

#[derive(Debug)]
struct Out {
    stream: UnixStream,
    outgoing: Option<Sender<EmbedderMessage>>,
    gates: Gates,
}

impl Link {
    pub(crate) fn new(stream: UnixStream, outgoing: Sender<EmbedderMessage>) -> Link {
        let mut gates = Gates::default();
        gates.hold(Target::Runtime);
        Link {
            out: Mutex::new(Out {
                stream,
                outgoing: Some(outgoing),
                gates,
            }),
            calls: Calls::default(),
        }
    }

    pub(crate) fn send<E: ForGecko>(&self, target: Target, payload: &E) {
        match EmbedderMessage::send(target, payload, None) {
            Ok(message) => self.write(message),
            Err(error) => tracing::error!(%error, message = E::NAME, "failed to encode message"),
        }
    }

    pub(crate) fn ask<E: Request + ForGecko>(
        &self,
        target: Target,
        payload: &E,
    ) -> Asked<E::Response> {
        let (callback, receiver) = match self.calls.place(target.clone()) {
            Ok(placed) => placed,
            Err(error) => return Asked::failed(error),
        };
        match EmbedderMessage::send(target, payload, Some(callback)) {
            Ok(message) => self.write(message),
            Err(error) => {
                self.calls
                    .resolve(callback, Err(Error::Malformed(error.to_string())));
            }
        }
        Asked::waiting(receiver)
    }

    pub(crate) fn answer(&self, callback: CallbackId, outcome: Result<Value, Value>) {
        let reply = match outcome {
            Ok(data) => Reply {
                callback,
                success: true,
                data: (!data.is_null()).then_some(data),
            },
            Err(error) => Reply::failure(callback, error),
        };
        self.write(EmbedderMessage::Reply(reply));
    }

    pub(crate) fn answer_with<T: Serialize>(&self, callback: CallbackId, response: &T) {
        match serde_json::to_value(response) {
            Ok(data) => self.answer(callback, Ok(data)),
            Err(error) => self.answer(callback, Err(Value::String(error.to_string()))),
        }
    }

    pub(crate) fn refuse(&self, callback: Option<CallbackId>, reason: impl Into<String>) {
        if let Some(callback) = callback {
            self.answer(callback, Err(Value::String(reason.into())));
        }
    }

    pub(crate) fn listen(&self, target: Target, events: &[&str]) {
        self.write(EmbedderMessage::Listen(Listen::new(target, events)));
    }

    pub(crate) fn open(&self, window: &WindowId, init: &InitData) {
        let message = match init.open(window.clone()) {
            Ok(message) => message,
            Err(error) => {
                tracing::error!(%error, "failed to encode window description");
                return;
            }
        };
        let mut out = self.out();
        out.gates.hold(Target::Window(window.clone()));
        if let Some(message) = out.gates.filter(message) {
            out.write_message(message);
        }
    }

    pub(crate) fn close(&self, window: &WindowId) {
        self.write(EmbedderMessage::Close(Close {
            window: window.clone(),
        }));
    }

    pub(crate) fn release_held(&self, target: &Target) {
        let mut out = self.out();
        for message in out.gates.release(target) {
            out.write_message(message);
        }
    }

    pub(crate) fn closed(&self, window: &WindowId) {
        let target = Target::Window(window.clone());
        drop(self.out().gates.release(&target));
        self.calls.fail_target(&target);
    }

    pub(crate) fn resolve(&self, reply: Reply) {
        let outcome = match reply.success {
            true => Ok(reply.data.unwrap_or(Value::Null)),
            false => Err(Error::Rejected(error_reason(reply.data))),
        };
        self.calls.resolve(reply.callback, outcome);
    }

    pub(crate) fn release(&self, callback: CallbackId) {
        self.calls.resolve(callback, Err(Error::NoReply));
    }

    pub(crate) fn disconnect(&self) {
        {
            let mut out = self.out();
            out.outgoing = None;
            if let Err(error) = out.stream.shutdown(Shutdown::Both) {
                tracing::debug!(%error, "failed to shut down engine socket");
            }
        }
        self.calls.fail_all();
    }

    pub(crate) fn is_disconnected(&self) -> bool {
        self.out().outgoing.is_none()
    }

    fn write(&self, message: EmbedderMessage) {
        let mut out = self.out();
        if let Some(message) = out.gates.filter(message) {
            out.write_message(message);
        }
    }

    fn out(&self) -> MutexGuard<'_, Out> {
        self.out.lock()
    }
}

impl Out {
    fn write_message(&mut self, message: EmbedderMessage) {
        if let Some(outgoing) = &self.outgoing
            && outgoing.send(message).is_err()
        {
            tracing::debug!("engine writer stopped");
        }
    }
}

fn error_reason(data: Option<Value>) -> String {
    match data {
        Some(Value::String(text)) => text,
        Some(Value::Object(object)) => object
            .get("message")
            .and_then(Value::as_str)
            .map_or_else(|| Value::Object(object.clone()).to_string(), str::to_owned),
        Some(other) => other.to_string(),
        None => "no reason given".to_owned(),
    }
}
