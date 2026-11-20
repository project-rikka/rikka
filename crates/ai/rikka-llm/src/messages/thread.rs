use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use rikka_net::{StatusCode, Url};
use rikka_sync::Mutex;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::event::Event;
use crate::failure::Failure;
use crate::messages::encode;
use crate::response::Response;
use crate::stop::Stop;
use crate::stream::Stream;

pub(super) const BETA: &str = "message-threads-2026-08-12";

const LIFETIME: Duration = Duration::from_secs(5 * 60);
const UNTHREADED_SPAN: Duration = Duration::from_secs(60 * 60);
const FREE_FIELDS: [&str; 4] = ["messages", "max_tokens", "stop_sequences", "stream"];
const CREATE_ONLY_FIELDS: [&str; 2] = ["system", "tools"];

type Hash = [u8; 32];

static HELD: LazyLock<Mutex<HashMap<Key, Held>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct Key {
    endpoint: String,
    conversation: String,
}

enum Held {
    Open(Open),
    Unthreaded { since: Instant },
}

pub(super) struct Open {
    id: String,
    fingerprint: Hash,
    covered: Vec<Hash>,
    sent: Instant,
    recovered: bool,
    paused: bool,
}

impl Held {
    fn is_live(&self) -> bool {
        match self {
            Held::Open(open) => open.sent.elapsed() < LIFETIME,
            Held::Unthreaded { since } => since.elapsed() < UNTHREADED_SPAN,
        }
    }
}

impl Open {
    pub(super) fn id(&self) -> &str {
        &self.id
    }

    pub(super) fn from(&self) -> usize {
        self.covered.len()
    }

    pub(super) fn is_recovered(&self) -> bool {
        self.recovered
    }
}

pub(super) enum Step {
    Create,
    Continue(Open),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Lapse {
    Missing,
    Stale,
    Unsupported,
}

pub(super) struct Ledger {
    fingerprint: Hash,
    messages: Vec<Hash>,
}

pub(super) struct Recording {
    key: Key,
    ledger: Ledger,
    sent: Instant,
    recovered: bool,
}

impl Key {
    pub(super) fn new(endpoint: &Url, conversation: &str) -> Key {
        Key {
            endpoint: endpoint.as_str().to_owned(),
            conversation: conversation.to_owned(),
        }
    }

    pub(super) fn is_unthreaded(&self) -> bool {
        let mut held = HELD.lock();
        held.retain(|_, entry| entry.is_live());
        matches!(held.get(self), Some(Held::Unthreaded { .. }))
    }

    pub(super) fn step(&self, ledger: &Ledger) -> Step {
        let Some(Held::Open(open)) = HELD.lock().remove(self) else {
            return Step::Create;
        };

        let extends = open.fingerprint == ledger.fingerprint
            && (open.paused || open.covered.len() < ledger.messages.len())
            && ledger.messages.starts_with(&open.covered);
        match extends {
            true => Step::Continue(open),
            false => Step::Create,
        }
    }

    pub(super) fn restore(&self, open: Open) {
        HELD.lock().insert(self.clone(), Held::Open(open));
    }

    pub(super) fn forsake(&self) {
        HELD.lock().insert(
            self.clone(),
            Held::Unthreaded {
                since: Instant::now(),
            },
        );
    }
}

impl Ledger {
    pub(super) fn new(body: &Value, betas: &str) -> Ledger {
        let mut fixed = Sha256::new();
        absorb(&mut fixed, betas.as_bytes());
        let fields = body.as_object().into_iter().flatten();
        for (name, value) in fields.filter(|(name, _)| !FREE_FIELDS.contains(&name.as_str())) {
            absorb(&mut fixed, name.as_bytes());
            absorb(&mut fixed, &hash(value));
        }

        let messages = body["messages"]
            .as_array()
            .map(|messages| messages.iter().map(hash).collect())
            .unwrap_or_default();

        Ledger {
            fingerprint: fixed.finalize().into(),
            messages,
        }
    }
}

impl Recording {
    pub(super) fn new(key: Key, ledger: Ledger, sent: Instant, recovered: bool) -> Recording {
        Recording {
            key,
            ledger,
            sent,
            recovered,
        }
    }

    pub(super) fn record(self, response: &Response) {
        if response.id().is_empty() || matches!(response.stop(), Stop::Declined(_)) {
            return;
        }

        let mut covered = self.ledger.messages;
        covered.extend(encode::reply(response.content()).as_ref().map(hash));
        HELD.lock().insert(
            self.key,
            Held::Open(Open {
                id: response.id().to_owned(),
                fingerprint: self.ledger.fingerprint,
                covered,
                sent: self.sent,
                recovered: self.recovered,
                paused: response.stop().is_paused(),
            }),
        );
    }
}

pub(super) fn recorded(stream: Stream, recording: Recording) -> Stream {
    let mut recording = Some(recording);
    Stream::new(stream.inspect(move |event| {
        if let Ok(Event::Done(response)) = event
            && let Some(recording) = recording.take()
        {
            recording.record(response);
        }
    }))
}

pub(super) fn creation(body: &mut Value) {
    body["thread"] = json!({ "type": "create" });
}

pub(super) fn unthreaded(body: &mut Value) {
    if let Some(fields) = body.as_object_mut() {
        fields.remove("thread");
    }
}

pub(super) fn continuation(body: &Value, from: usize, id: &str) -> Value {
    let mut continued = Map::new();
    for (name, value) in body.as_object().into_iter().flatten() {
        match name.as_str() {
            "messages" => {
                let fresh = value.as_array().map(|messages| messages[from..].to_vec());
                continued.insert(name.clone(), Value::Array(fresh.unwrap_or_default()));
            }
            name if CREATE_ONLY_FIELDS.contains(&name) => {}
            _ => {
                continued.insert(name.clone(), value.clone());
            }
        }
    }
    continued.insert(
        "thread".to_owned(),
        json!({ "type": "continue", "previous_message_id": id }),
    );
    Value::Object(continued)
}

pub(super) fn lapse(failure: &Failure) -> Option<Lapse> {
    let refusal = failure.refusal()?;
    match refusal.code.as_str() {
        "thread_not_found" => Some(Lapse::Missing),
        "thread_already_continued"
        | "thread_continue_in_flight"
        | "thread_fingerprint_mismatch" => Some(Lapse::Stale),
        "thread_unsupported_request" => Some(Lapse::Unsupported),
        _ if refusal.status == StatusCode::BAD_REQUEST && refusal.message.contains(BETA) => {
            Some(Lapse::Unsupported)
        }
        _ => None,
    }
}

fn hash(value: &Value) -> Hash {
    let mut hashing = Hashing(Sha256::new());
    serde_json::to_writer(&mut hashing, value).expect("a JSON value serializes into a hasher");
    hashing.0.finalize().into()
}

fn absorb(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

struct Hashing(Sha256);

impl Write for Hashing {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
