use base64::Engine;
use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use rikka_encoding::{Decoder, Encoding};
use serde::Deserialize;

use crate::address::Address;
use crate::words;

pub const INBOX: &str = "INBOX";
pub const UNREAD: &str = "UNREAD";
pub const STARRED: &str = "STARRED";
pub const IMPORTANT: &str = "IMPORTANT";
pub const SENT: &str = "SENT";
pub const DRAFT: &str = "DRAFT";
pub const SPAM: &str = "SPAM";
pub const TRASH: &str = "TRASH";

const MIXED: &str = "multipart/mixed";
const PLAIN: &str = "text/plain";
const HTML: &str = "text/html";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub id: String,
    pub history_id: u64,
    pub subject: String,
    pub snippet: String,
    pub senders: Vec<Address>,
    pub date_ms: i64,
    pub messages: usize,
    pub labels: Vec<String>,
    pub has_attachment: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attachment {
    pub message_id: String,
    pub attachment_id: String,
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub content_id: Option<String>,
    pub inline: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Message {
    pub id: String,
    pub thread_id: String,
    pub labels: Vec<String>,
    pub from: Address,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    pub reply_to: Vec<Address>,
    pub subject: String,
    pub snippet: String,
    pub date_ms: i64,
    pub message_id: String,
    pub references: Vec<String>,
    pub text: Option<String>,
    pub html: Option<String>,
    pub attachments: Vec<Attachment>,
    pub(crate) unloaded: Vec<Unloaded>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Thread {
    pub id: String,
    pub history_id: u64,
    pub messages: Vec<Message>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Unloaded {
    pub(crate) attachment_id: String,
    pub(crate) charset: Option<String>,
    pub(crate) html: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawThread {
    id: String,
    #[serde(default)]
    history_id: String,
    #[serde(default)]
    messages: Vec<RawMessage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMessage {
    id: String,
    #[serde(default)]
    thread_id: String,
    #[serde(default)]
    label_ids: Vec<String>,
    #[serde(default)]
    snippet: String,
    #[serde(default)]
    internal_date: String,
    payload: Option<RawPart>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPart {
    #[serde(default)]
    mime_type: String,
    #[serde(default)]
    filename: String,
    #[serde(default)]
    headers: Vec<RawHeader>,
    body: Option<RawBody>,
    #[serde(default)]
    parts: Vec<RawPart>,
}

#[derive(Deserialize)]
struct RawHeader {
    name: String,
    value: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawBody {
    attachment_id: Option<String>,
    #[serde(default)]
    size: u64,
    data: Option<String>,
}

impl Summary {
    #[must_use]
    pub fn has(&self, label: &str) -> bool {
        self.labels.iter().any(|held| held == label)
    }

    #[must_use]
    pub fn is_unread(&self) -> bool {
        self.has(UNREAD)
    }

    #[must_use]
    pub fn is_starred(&self) -> bool {
        self.has(STARRED)
    }

    pub fn relabel(&mut self, add: &[&str], remove: &[&str]) {
        self.labels.retain(|held| !remove.contains(&held.as_str()));
        for label in add {
            if !self.has(label) {
                self.labels.push((*label).to_owned());
            }
        }
    }
}

impl Message {
    #[must_use]
    pub fn has(&self, label: &str) -> bool {
        self.labels.iter().any(|held| held == label)
    }

    #[must_use]
    pub fn is_draft(&self) -> bool {
        self.has(DRAFT)
    }

    #[must_use]
    pub fn inline_picture(&self, content_id: &str) -> Option<&Attachment> {
        let wanted = content_id.trim_start_matches("cid:");
        self.attachments
            .iter()
            .find(|attachment| attachment.content_id.as_deref() == Some(wanted))
    }

    pub fn files(&self) -> impl Iterator<Item = &Attachment> {
        self.attachments
            .iter()
            .filter(|attachment| !attachment.inline)
    }
}

impl RawThread {
    pub(crate) fn summary(self) -> Option<Summary> {
        let history_id = self.history_id.parse().unwrap_or(0);
        let last = self.messages.last()?;
        let mut labels: Vec<String> = Vec::new();
        let mut senders: Vec<Address> = Vec::new();
        let mut has_attachment = false;
        for message in &self.messages {
            for label in &message.label_ids {
                if !labels.contains(label) {
                    labels.push(label.clone());
                }
            }
            let Some(payload) = &message.payload else {
                continue;
            };
            has_attachment |= payload.mime_type == MIXED;
            if let Some(from) = payload.header("From").and_then(Address::parse)
                && !senders.iter().any(|known| known.email == from.email)
            {
                senders.push(from);
            }
        }
        let subject = self
            .messages
            .first()
            .and_then(|first| first.payload.as_ref())
            .and_then(|payload| payload.header("Subject"))
            .map(words::decode)
            .unwrap_or_default();
        Some(Summary {
            subject,
            snippet: rikka_html::decode(&last.snippet).into_owned(),
            senders,
            date_ms: last.internal_date.parse().unwrap_or(0),
            messages: self.messages.len(),
            labels,
            has_attachment,
            history_id,
            id: self.id,
        })
    }

    pub(crate) fn thread(self) -> Thread {
        Thread {
            history_id: self.history_id.parse().unwrap_or(0),
            messages: self.messages.into_iter().map(RawMessage::message).collect(),
            id: self.id,
        }
    }
}

impl RawMessage {
    fn message(self) -> Message {
        let mut message = Message {
            thread_id: self.thread_id,
            labels: self.label_ids,
            snippet: rikka_html::decode(&self.snippet).into_owned(),
            date_ms: self.internal_date.parse().unwrap_or(0),
            ..Message::default()
        };
        if let Some(payload) = &self.payload {
            let addresses = |name: &str| {
                payload
                    .header(name)
                    .map(Address::parse_list)
                    .unwrap_or_default()
            };
            message.from = payload
                .header("From")
                .and_then(Address::parse)
                .unwrap_or_default();
            message.to = addresses("To");
            message.cc = addresses("Cc");
            message.reply_to = addresses("Reply-To");
            message.subject = payload
                .header("Subject")
                .map(words::decode)
                .unwrap_or_default();
            message.message_id = payload
                .header("Message-ID")
                .unwrap_or_default()
                .trim()
                .to_owned();
            message.references = payload
                .header("References")
                .unwrap_or_default()
                .split_whitespace()
                .map(str::to_owned)
                .collect();
            payload.collect(&self.id, &mut message);
        }
        message.id = self.id;
        message
    }
}

impl RawPart {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|header| header.name.eq_ignore_ascii_case(name))
            .map(|header| header.value.as_str())
    }

    fn collect(&self, message_id: &str, message: &mut Message) {
        if self.mime_type.starts_with("multipart/") {
            for part in &self.parts {
                part.collect(message_id, message);
            }
            return;
        }
        let body = self.body.as_ref();
        let attachment_id = body.and_then(|body| body.attachment_id.clone());
        let is_text = self.filename.is_empty() && [PLAIN, HTML].contains(&self.mime_type.as_str());
        let html = self.mime_type == HTML;
        let taken = match html {
            true => message.html.is_some(),
            false => message.text.is_some(),
        };
        if is_text && !taken {
            let charset = self
                .header("Content-Type")
                .and_then(|content_type| parameter(content_type, "charset"));
            match (body.and_then(|body| body.data.as_deref()), attachment_id) {
                (Some(data), _) => {
                    let text = decoded(data).map(|bytes| text(bytes, charset.as_deref()));
                    match html {
                        true => message.html = text,
                        false => message.text = text,
                    }
                }
                (None, Some(attachment_id)) => message.unloaded.push(Unloaded {
                    attachment_id,
                    charset,
                    html,
                }),
                (None, None) => {}
            }
            return;
        }
        let Some(attachment_id) = attachment_id else {
            return;
        };
        let content_id = self
            .header("Content-ID")
            .map(|id| id.trim().trim_matches(['<', '>']).to_owned())
            .filter(|id| !id.is_empty());
        let disposition = self.header("Content-Disposition").unwrap_or_default();
        let inline = content_id.is_some()
            && self.mime_type.starts_with("image/")
            && !disposition.trim_start().starts_with("attachment");
        message.attachments.push(Attachment {
            message_id: message_id.to_owned(),
            attachment_id,
            name: words::decode(&self.filename),
            mime: self.mime_type.clone(),
            size: body.map_or(0, |body| body.size),
            content_id,
            inline,
        });
    }
}

pub(crate) fn decoded(data: &str) -> Option<Vec<u8>> {
    URL_SAFE_NO_PAD
        .decode(data.trim_end_matches('='))
        .or_else(|_| URL_SAFE.decode(data))
        .ok()
}

pub(crate) fn text(bytes: Vec<u8>, charset: Option<&str>) -> String {
    match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => {
            let declared = charset.and_then(|charset| Encoding::for_label(charset.as_bytes()));
            Decoder::new()
                .declared(declared)
                .decode(error.into_bytes())
                .text
        }
    }
}

fn parameter(header: &str, name: &str) -> Option<String> {
    header.split(';').skip(1).find_map(|parameter| {
        let (key, value) = parameter.split_once('=')?;
        key.trim()
            .eq_ignore_ascii_case(name)
            .then(|| value.trim().trim_matches('"').to_owned())
    })
}
