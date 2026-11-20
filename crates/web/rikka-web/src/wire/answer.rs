use std::fmt;
use std::sync::Arc;

use rikka_gecko_protocol::CallbackId;
use serde::Serialize;
use serde_json::Value;

use crate::wire::link::Link;

pub(crate) struct Answer {
    link: Arc<Link>,
    callback: Option<CallbackId>,
    fallback: Option<Result<Value, Value>>,
}

impl Answer {
    pub(crate) fn new(
        link: &Arc<Link>,
        callback: Option<CallbackId>,
        fallback: Result<Value, Value>,
    ) -> Answer {
        Answer {
            link: Arc::clone(link),
            callback,
            fallback: Some(fallback),
        }
    }

    pub(crate) fn fallback_to<T: Serialize>(
        link: &Arc<Link>,
        callback: Option<CallbackId>,
        fallback: &T,
    ) -> Answer {
        let fallback =
            serde_json::to_value(fallback).map_err(|error| Value::String(error.to_string()));
        Answer::new(link, callback, fallback)
    }

    pub(crate) fn with<T: Serialize>(mut self, response: &T) {
        self.fallback = None;
        if let Some(callback) = self.callback.take() {
            self.link.answer_with(callback, response);
        }
    }

    pub(crate) fn refuse(mut self, reason: impl Into<String>) {
        self.fallback = None;
        if let Some(callback) = self.callback.take() {
            self.link
                .answer(callback, Err(Value::String(reason.into())));
        }
    }

    pub(crate) fn link(&self) -> &Arc<Link> {
        &self.link
    }
}

impl Drop for Answer {
    fn drop(&mut self) {
        if let (Some(callback), Some(fallback)) = (self.callback.take(), self.fallback.take()) {
            self.link.answer(callback, fallback);
        }
    }
}

impl fmt::Debug for Answer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Answer")
            .field("callback", &self.callback.map(CallbackId::get))
            .finish()
    }
}
