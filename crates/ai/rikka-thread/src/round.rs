use std::sync::Arc;

use rikka_llm::Value;

use crate::call::{Call, CallResult};
use crate::replay::Replay;

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Round {
    #[serde(default, rename = "said")]
    text: String,
    #[serde(default)]
    calls: Vec<Call>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    notices: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    replay: Option<Replay>,
}

impl Round {
    pub(crate) fn say(&mut self, more: &str) {
        self.text.push_str(more);
    }

    pub(crate) fn started(&mut self, id: &Arc<str>, tool: &Arc<str>, input: &Arc<Value>) {
        self.calls.push(Call::new(
            Arc::clone(id),
            Arc::clone(tool),
            Arc::clone(input),
        ));
    }

    pub(crate) fn add(&mut self, call: Call) {
        self.calls.push(call);
    }

    pub(crate) fn set_result(&mut self, id: &str, result: CallResult) {
        if let Some(call) = self.call(id) {
            call.set_result(result);
        }
    }

    pub(crate) fn call(&mut self, id: &str) -> Option<&mut Call> {
        self.calls.iter_mut().find(|call| call.id() == id)
    }

    pub(crate) fn replayed(&mut self, replay: &Replay) {
        self.replay = Some(match self.replay.take() {
            Some(earlier) => earlier.then(replay),
            None => replay.clone(),
        });
    }

    pub(crate) fn replay(&self) -> Option<&Replay> {
        self.replay.as_ref()
    }

    pub(crate) fn bytes(&self) -> usize {
        let notices: usize = self.notices.iter().map(String::len).sum();
        let calls: usize = self
            .calls
            .iter()
            .map(|call| {
                call.id().len()
                    + call.tool().len()
                    + call.output().unwrap_or_default().len()
                    + worth(call.input())
                    + call
                        .pictured()
                        .iter()
                        .chain(call.displayed())
                        .map(|it| it.data().len())
                        .sum::<usize>()
            })
            .sum();
        let replay = self.replay.as_ref().map_or(0, Replay::bytes);
        self.text.len() + notices + calls + replay
    }

    pub(crate) fn noticed(&mut self, notice: String) {
        self.notices.push(notice);
    }

    pub(crate) fn settle(&mut self) {
        self.calls.retain(|call| !call.is_running());
    }

    pub(crate) fn into_spoken(self) -> (String, Option<Replay>) {
        (self.text, self.replay)
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn calls(&self) -> &[Call] {
        &self.calls
    }

    #[must_use]
    pub fn notices(&self) -> &[String] {
        &self.notices
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
            && self.calls.is_empty()
            && self.notices.is_empty()
            && self.replay.is_none()
    }

    #[must_use]
    pub fn is_answer(&self) -> bool {
        self.calls.is_empty() && self.notices.is_empty()
    }
}

pub(crate) fn worth(value: &Value) -> usize {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => 8,
        Value::String(text) => text.len(),
        Value::Array(items) => items.iter().map(worth).sum::<usize>() + 8,
        Value::Object(fields) => {
            fields
                .iter()
                .map(|(name, value)| name.len() + worth(value))
                .sum::<usize>()
                + 8
        }
    }
}
