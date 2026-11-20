use std::sync::Arc;
use std::sync::mpsc::{self, SyncSender};

use base64::alphabet;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig, STANDARD};
use base64::engine::{DecodePaddingMode, Engine};

use crate::clipboard::access::ClipboardAccess;
use crate::clipboard::scan::Osc52;
use crate::clipboard::selection;
use crate::clipboard::target::Target;

pub(crate) type Reply = Arc<dyn Fn(Vec<u8>) + Send + Sync>;

const ANSWER: &[u8] = b"\x1b]52;";
const BELL: &[u8] = b"\x07";
const STRING_END: &[u8] = b"\x1b\\";
const QUERY: &[u8] = b"?";
const DEFAULT: u8 = b'c';

const WAITING: usize = 8;

const SENT: GeneralPurpose = GeneralPurpose::new(
    &alphabet::STANDARD,
    GeneralPurposeConfig::new()
        .with_decode_padding_mode(DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true),
);

pub(crate) struct Requests {
    access: ClipboardAccess,
    reply: Reply,
    queries: Option<SyncSender<Query>>,
}

#[derive(Debug)]
struct Query {
    spec: SelectionSpec,
    bell: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SelectionSpec {
    targets: Vec<Target>,
    echo: Vec<u8>,
}

impl Requests {
    pub(crate) fn new(access: ClipboardAccess, reply: Reply) -> Self {
        Self {
            access,
            reply,
            queries: None,
        }
    }

    pub(crate) fn handle(&mut self, sequence: Osc52) {
        match sequence {
            Osc52::Body { body, bell } => self.act(&body, bell),
            Osc52::TooLarge => {
                tracing::warn!("OSC 52 payload exceeds clipboard limit");
            }
        }
    }

    fn act(&mut self, body: &[u8], bell: bool) {
        let Some((spec, data)) = split(body) else {
            return;
        };
        match data == QUERY {
            true => self.answer(spec, bell),
            false => self.store(&spec.targets, data),
        }
    }

    fn store(&self, targets: &[Target], data: &[u8]) {
        if !self.access.writes() {
            return;
        }
        let sequence = SENT.decode(data).unwrap_or_default();
        for target in targets {
            target.store(&sequence);
        }
    }

    fn answer(&mut self, spec: SelectionSpec, bell: bool) {
        if !self.access.reads() {
            return;
        }
        let reply = &self.reply;
        let queries = self.queries.get_or_insert_with(|| spawn_reader(reply));
        if queries.try_send(Query { spec, bell }).is_err() {
            tracing::warn!("OSC 52 query queue full, dropping request");
        }
    }
}

fn spawn_reader(reply: &Reply) -> SyncSender<Query> {
    let (sender, receiver) = mpsc::sync_channel::<Query>(WAITING);
    let reply = Arc::clone(reply);
    selection::away(move || {
        for query in receiver {
            let contents = query
                .spec
                .targets
                .iter()
                .map(|target| target.contents())
                .find(|contents| !contents.is_empty())
                .unwrap_or_default();
            reply(message(&query.spec.echo, &contents, query.bell));
        }
    });
    sender
}

impl SelectionSpec {
    fn from_name(named: &[u8]) -> Option<Self> {
        let named = match named.is_empty() {
            true => &[DEFAULT][..],
            false => named,
        };
        let mut spec = Self {
            targets: Vec::new(),
            echo: Vec::new(),
        };
        for byte in named.iter().copied() {
            let Some(target) = Target::from_byte(byte) else {
                continue;
            };
            if spec.targets.contains(&target) {
                continue;
            }
            spec.targets.push(target);
            spec.echo.push(byte);
        }
        (!spec.targets.is_empty()).then_some(spec)
    }
}

fn split(body: &[u8]) -> Option<(SelectionSpec, &[u8])> {
    let at = body.iter().position(|byte| *byte == b';')?;
    Some((SelectionSpec::from_name(&body[..at])?, &body[at + 1..]))
}

fn message(echo: &[u8], data: &[u8], bell: bool) -> Vec<u8> {
    let mut message = Vec::from(ANSWER);
    message.extend_from_slice(echo);
    message.push(b';');
    message.extend_from_slice(STANDARD.encode(data).as_bytes());
    message.extend_from_slice(match bell {
        true => BELL,
        false => STRING_END,
    });
    message
}
