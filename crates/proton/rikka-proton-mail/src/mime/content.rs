use std::fmt;

use rikka_proton_pgp::SignatureStatus;

use crate::attachment::Disposition;
use crate::mime::headers::Headers;
use crate::mime::html_text::html_to_text;
use crate::mime::part::{MimePart, Multipart, PartBody, Protection, normalize_line_endings};

const PROTECTED_HEADERS: &str = "protected-headers";
const DEFAULT_FILE_STEM: &str = "attachment";
const EXTENSIONS: &[(&str, &str)] = &[
    ("image/jpeg", "jpg"),
    ("image/png", "png"),
    ("image/gif", "gif"),
    ("image/webp", "webp"),
    ("image/svg+xml", "svg"),
    ("image/bmp", "bmp"),
    ("application/pdf", "pdf"),
    ("text/calendar", "ics"),
    ("text/plain", "txt"),
    ("text/html", "html"),
    ("application/pgp-keys", "asc"),
    ("application/pgp-signature", "asc"),
    ("application/pgp-encrypted", "asc"),
];

#[derive(Clone)]
pub struct MimeFile {
    name: String,
    mime_type: String,
    content_id: Option<String>,
    disposition: Disposition,
    data: Vec<u8>,
}

impl MimeFile {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn mime_type(&self) -> &str {
        &self.mime_type
    }

    #[must_use]
    pub fn content_id(&self) -> Option<&str> {
        self.content_id.as_deref()
    }

    #[must_use]
    pub fn disposition(&self) -> &Disposition {
        &self.disposition
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }

    fn from_part(part: &MimePart) -> MimeFile {
        let essence = part.content_type().essence();
        let name = match (part.file_name(), part.embedded()) {
            (Some(name), _) => name.to_owned(),
            (None, Some(embedded)) => embedded_message_file_name(embedded),
            (None, None) => default_file_name(&essence),
        };

        MimeFile {
            name,
            mime_type: essence,
            content_id: part.content_id(),
            disposition: part.disposition_kind().cloned().unwrap_or_default(),
            data: part.data().map(<[u8]>::to_vec).unwrap_or_default(),
        }
    }
}

impl fmt::Debug for MimeFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MimeFile")
            .field("name", &self.name)
            .field("mime_type", &self.mime_type)
            .field("content_id", &self.content_id)
            .field("disposition", &self.disposition)
            .field("bytes", &self.data.len())
            .finish()
    }
}

#[derive(Clone, Default)]
pub struct MimeContent {
    html: Option<String>,
    text: String,
    inline: Vec<MimeFile>,
    attachments: Vec<MimeFile>,
    signature: Option<SignatureStatus>,
    protected: Option<Headers>,
}

impl MimeContent {
    pub(crate) fn from_html(html: String) -> MimeContent {
        MimeContent {
            text: html_to_text(&html),
            html: Some(html),
            ..MimeContent::default()
        }
    }

    pub(crate) fn from_text(text: String) -> MimeContent {
        MimeContent {
            text: normalize_line_endings(&text),
            ..MimeContent::default()
        }
    }

    pub(crate) fn from_part(root: &MimePart) -> MimeContent {
        let mut collector = Collector::default();
        collector.walk(root, PartPosition::Root);

        let shown = displayed_root(root);
        let protected = [root, shown]
            .into_iter()
            .find(|part| part.content_type().parameter(PROTECTED_HEADERS).is_some())
            .map(|part| part.headers().clone())
            .or(collector.header_part.take());

        MimeContent {
            html: collector.html(),
            text: collector.text(),
            inline: collector.inline,
            attachments: collector.attachments,
            signature: collector.signature,
            protected,
        }
    }

    #[must_use]
    pub fn html(&self) -> Option<&str> {
        self.html.as_deref()
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn inline(&self) -> &[MimeFile] {
        &self.inline
    }

    #[must_use]
    pub fn inline_file(&self, content_id: &str) -> Option<&MimeFile> {
        let target = content_id
            .trim()
            .trim_start_matches("cid:")
            .trim_start_matches('<')
            .trim_end_matches('>');
        self.inline
            .iter()
            .chain(&self.attachments)
            .find(|file| file.content_id() == Some(target))
    }

    #[must_use]
    pub fn attachments(&self) -> &[MimeFile] {
        &self.attachments
    }

    #[must_use]
    pub fn signature(&self) -> Option<&SignatureStatus> {
        self.signature.as_ref()
    }

    #[must_use]
    pub fn protected_headers(&self) -> Option<&Headers> {
        self.protected.as_ref()
    }

    #[must_use]
    pub fn subject(&self) -> Option<String> {
        self.protected
            .as_ref()
            .and_then(|headers| headers.get("subject"))
    }
}

impl fmt::Debug for MimeContent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MimeContent")
            .field("html", &self.html.as_ref().map(String::len))
            .field("text", &self.text.len())
            .field("inline", &self.inline)
            .field("attachments", &self.attachments)
            .field("signature", &self.signature)
            .field("protected", &self.protected.is_some())
            .finish()
    }
}

enum Segment {
    Html(String),
    Plain(String),
    Both { html: String, plain: String },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PartPosition {
    Root,
    Nested,
}

#[derive(Default)]
struct Collector {
    segments: Vec<Segment>,
    inline: Vec<MimeFile>,
    attachments: Vec<MimeFile>,
    signature: Option<SignatureStatus>,
    has_signature: bool,
    header_part: Option<Headers>,
}

impl Collector {
    fn walk(&mut self, part: &MimePart, position: PartPosition) {
        match part.body() {
            PartBody::Multipart(multipart) => self.multipart(part, multipart),
            PartBody::Message { .. } => self.attachments.push(MimeFile::from_part(part)),
            PartBody::Leaf(data) => self.leaf(part, data, position),
        }
    }

    fn multipart(&mut self, part: &MimePart, multipart: &Multipart) {
        match &multipart.protection {
            Protection::Signed { status, .. } => {
                if !self.has_signature {
                    self.has_signature = true;
                    self.signature.clone_from(status);
                }
                if let Some(first) = multipart.parts.first() {
                    self.walk(first, PartPosition::Nested);
                }
            }
            Protection::Encrypted {
                decrypted, status, ..
            } => match decrypted {
                Some(decrypted) => {
                    if !self.has_signature
                        && status.as_ref().is_some_and(|status| !status.is_missing())
                    {
                        self.has_signature = true;
                        self.signature.clone_from(status);
                    }
                    self.walk(decrypted, PartPosition::Nested);
                }
                None => {
                    for child in &multipart.parts {
                        self.attachments.push(MimeFile::from_part(child));
                    }
                }
            },
            Protection::Plain => match part.content_type().subtype() {
                "alternative" => self.alternative(&multipart.parts),
                "related" => self.related(part, &multipart.parts),
                _ => {
                    for child in &multipart.parts {
                        self.walk(child, PartPosition::Nested);
                    }
                }
            },
        }
    }

    fn alternative(&mut self, parts: &[MimePart]) {
        let html = parts.iter().rev().find(|part| renders_html(part));
        let plain = parts
            .iter()
            .rev()
            .find(|part| is_body_text(part, "text/plain"));

        match (html, plain) {
            (Some(html), Some(plain)) => {
                let mut rich = Collector::default();
                rich.walk(html, PartPosition::Nested);
                let mut simple = Collector::default();
                simple.walk(plain, PartPosition::Nested);

                self.segments.push(Segment::Both {
                    html: rich.html().unwrap_or_default(),
                    plain: simple.text(),
                });
                self.merge_rich(rich);
            }
            (Some(selected), None) | (None, Some(selected)) => {
                self.walk(selected, PartPosition::Nested)
            }
            (None, None) => {
                if let Some(last) = parts.last() {
                    self.walk(last, PartPosition::Nested);
                }
            }
        }
    }

    fn related(&mut self, part: &MimePart, parts: &[MimePart]) {
        let start = part
            .content_type()
            .parameter("start")
            .map(|start| start.trim().trim_start_matches('<').trim_end_matches('>'));
        let root = start
            .and_then(|start| {
                parts
                    .iter()
                    .position(|child| child.content_id().as_deref() == Some(start))
            })
            .unwrap_or(0);

        for (index, child) in parts.iter().enumerate() {
            if index == root {
                self.walk(child, PartPosition::Nested);
                continue;
            }
            match child.body() {
                PartBody::Leaf(_)
                    if child.content_id().is_some()
                        || child.headers().contains("content-location") =>
                {
                    self.inline.push(MimeFile::from_part(child));
                }
                PartBody::Leaf(_) | PartBody::Message { .. } => {
                    self.attachments.push(MimeFile::from_part(child));
                }
                _ => self.walk(child, PartPosition::Nested),
            }
        }
    }

    fn leaf(&mut self, part: &MimePart, data: &[u8], position: PartPosition) {
        let content_type = part.content_type();

        if content_type.is("text/rfc822-headers")
            || (position == PartPosition::Nested
                && content_type.is_text()
                && content_type.parameter(PROTECTED_HEADERS).is_some())
        {
            if self.header_part.is_none() {
                self.header_part = Some(Headers::parse(data));
            }
            return;
        }

        if is_body_text(part, "text/html") {
            self.segments
                .push(Segment::Html(part.text().unwrap_or_default()));
            return;
        }
        if is_body_text(part, "text/plain") || is_body_text(part, "text/enriched") {
            self.segments
                .push(Segment::Plain(part.text().unwrap_or_default()));
            return;
        }

        let file = MimeFile::from_part(part);
        let embeddable =
            file.content_id.is_some() && !part.is_attachment() && content_type.kind() == "image";
        match embeddable {
            true => self.inline.push(file),
            false => self.attachments.push(file),
        }
    }

    fn merge_rich(&mut self, other: Collector) {
        self.inline.extend(other.inline);
        self.attachments.extend(other.attachments);
        if !self.has_signature && other.has_signature {
            self.has_signature = true;
            self.signature = other.signature;
        }
        if self.header_part.is_none() {
            self.header_part = other.header_part;
        }
    }

    fn html(&self) -> Option<String> {
        let rich = self
            .segments
            .iter()
            .any(|segment| matches!(segment, Segment::Html(_) | Segment::Both { .. }));
        if !rich {
            return None;
        }

        let pieces: Vec<String> = self
            .segments
            .iter()
            .map(|segment| match segment {
                Segment::Html(html) | Segment::Both { html, .. } => html.clone(),
                Segment::Plain(plain) => escape_html(plain),
            })
            .collect();
        Some(pieces.join("\n"))
    }

    fn text(&self) -> String {
        let pieces: Vec<String> = self
            .segments
            .iter()
            .map(|segment| match segment {
                Segment::Html(html) => html_to_text(html),
                Segment::Plain(plain) | Segment::Both { plain, .. } => plain.clone(),
            })
            .collect();
        pieces.join("\n")
    }
}

fn is_body_text(part: &MimePart, essence: &str) -> bool {
    matches!(part.body(), PartBody::Leaf(_))
        && part.content_type().is(essence)
        && !part.is_attachment()
        && part.file_name().is_none()
}

fn renders_html(part: &MimePart) -> bool {
    match part.body() {
        PartBody::Leaf(_) => is_body_text(part, "text/html"),
        PartBody::Multipart(multipart) => match &multipart.protection {
            Protection::Encrypted { decrypted, .. } => {
                decrypted.as_deref().is_some_and(renders_html)
            }
            _ => multipart.parts.iter().any(renders_html),
        },
        _ => false,
    }
}

fn displayed_root(part: &MimePart) -> &MimePart {
    match part
        .multipart()
        .map(|multipart| (&multipart.protection, &multipart.parts))
    {
        Some((Protection::Signed { .. }, parts)) => parts.first().map_or(part, displayed_root),
        Some((
            Protection::Encrypted {
                decrypted: Some(decrypted),
                ..
            },
            _,
        )) => displayed_root(decrypted),
        _ => part,
    }
}

pub(crate) fn part_signature(part: &MimePart) -> Option<&SignatureStatus> {
    match &part.multipart()?.protection {
        Protection::Signed { status, .. } => status.as_ref(),
        Protection::Encrypted {
            decrypted: Some(decrypted),
            status,
            ..
        } => status
            .as_ref()
            .filter(|status| !status.is_missing())
            .or_else(|| part_signature(decrypted)),
        _ => None,
    }
}

fn escape_html(plain: &str) -> String {
    let mut output = String::with_capacity(plain.len() + plain.len() / 8);
    for character in plain.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\n' => output.push_str("<br>\n"),
            other => output.push(other),
        }
    }
    output
}

fn embedded_message_file_name(embedded: &MimePart) -> String {
    let subject = embedded.headers().get("subject").unwrap_or_default();
    let sanitized: String = subject
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            other if other.is_control() => '_',
            other => other,
        })
        .collect();
    match sanitized.trim() {
        "" => "message.eml".to_owned(),
        named => format!("{named}.eml"),
    }
}

fn default_file_name(essence: &str) -> String {
    match EXTENSIONS
        .iter()
        .find(|(extension_type, _)| *extension_type == essence)
    {
        Some((_, extension)) => format!("{DEFAULT_FILE_STEM}.{extension}"),
        None => DEFAULT_FILE_STEM.to_owned(),
    }
}
