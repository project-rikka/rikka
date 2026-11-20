use std::fmt;

use rikka_proton_pgp::SignatureStatus;

use crate::attachment::Disposition;
use crate::mime::decode_charset;
use crate::mime::headers::Headers;
use crate::mime::parameters::{ContentDisposition, ContentType};

#[derive(Clone)]
pub struct MimePart {
    pub(crate) headers: Headers,
    pub(crate) content_type: ContentType,
    pub(crate) disposition: Option<ContentDisposition>,
    pub(crate) body: PartBody,
}

#[derive(Clone)]
#[non_exhaustive]
pub enum PartBody {
    Leaf(Vec<u8>),
    Multipart(Multipart),
    Message {
        raw: Vec<u8>,
        message: Box<MimePart>,
    },
}

#[derive(Clone)]
pub struct Multipart {
    pub(crate) parts: Vec<MimePart>,
    pub(crate) protection: Protection,
}

#[derive(Clone)]
pub(crate) enum Protection {
    Plain,
    Signed {
        signed: Vec<u8>,
        signature: Vec<u8>,
        status: Option<SignatureStatus>,
    },
    Encrypted {
        payload: Vec<u8>,
        decrypted: Option<Box<MimePart>>,
        status: Option<SignatureStatus>,
    },
}

impl MimePart {
    #[must_use]
    pub fn headers(&self) -> &Headers {
        &self.headers
    }

    #[must_use]
    pub fn content_type(&self) -> &ContentType {
        &self.content_type
    }

    #[must_use]
    pub fn disposition(&self) -> Option<&ContentDisposition> {
        self.disposition.as_ref()
    }

    #[must_use]
    pub fn body(&self) -> &PartBody {
        &self.body
    }

    #[must_use]
    pub fn data(&self) -> Option<&[u8]> {
        match &self.body {
            PartBody::Leaf(data) => Some(data),
            PartBody::Message { raw, .. } => Some(raw),
            PartBody::Multipart(_) => None,
        }
    }

    #[must_use]
    pub fn text(&self) -> Option<String> {
        match &self.body {
            PartBody::Leaf(data) if self.content_type.is_text() => Some(normalize_line_endings(
                &decode_charset(data, self.content_type.charset()),
            )),
            _ => None,
        }
    }

    #[must_use]
    pub fn children(&self) -> &[MimePart] {
        match &self.body {
            PartBody::Multipart(multipart) => &multipart.parts,
            _ => &[],
        }
    }

    #[must_use]
    pub fn file_name(&self) -> Option<&str> {
        self.disposition
            .as_ref()
            .and_then(ContentDisposition::file_name)
            .or_else(|| {
                self.content_type
                    .parameter("name")
                    .filter(|name| !name.is_empty())
            })
    }

    #[must_use]
    pub fn content_id(&self) -> Option<String> {
        self.headers
            .message_id("content-id")
            .filter(|id| !id.is_empty())
    }

    #[must_use]
    pub fn disposition_kind(&self) -> Option<&Disposition> {
        self.disposition.as_ref().map(ContentDisposition::kind)
    }

    #[must_use]
    pub fn is_attachment(&self) -> bool {
        self.disposition
            .as_ref()
            .is_some_and(ContentDisposition::is_attachment)
    }

    #[must_use]
    pub fn multipart(&self) -> Option<&Multipart> {
        match &self.body {
            PartBody::Multipart(multipart) => Some(multipart),
            _ => None,
        }
    }

    #[must_use]
    pub fn embedded(&self) -> Option<&MimePart> {
        match &self.body {
            PartBody::Message { message, .. } => Some(message),
            _ => None,
        }
    }
}

impl Multipart {
    #[must_use]
    pub fn parts(&self) -> &[MimePart] {
        &self.parts
    }

    #[must_use]
    pub fn is_signed(&self) -> bool {
        matches!(self.protection, Protection::Signed { .. })
    }

    #[must_use]
    pub fn is_encrypted(&self) -> bool {
        matches!(self.protection, Protection::Encrypted { .. })
    }

    #[must_use]
    pub fn signature(&self) -> Option<&SignatureStatus> {
        match &self.protection {
            Protection::Plain => None,
            Protection::Signed { status, .. } | Protection::Encrypted { status, .. } => {
                status.as_ref()
            }
        }
    }

    #[must_use]
    pub fn decrypted(&self) -> Option<&MimePart> {
        match &self.protection {
            Protection::Encrypted { decrypted, .. } => decrypted.as_deref(),
            _ => None,
        }
    }
}

impl fmt::Debug for MimePart {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MimePart")
            .field("content_type", &self.content_type.essence())
            .field("disposition", &self.disposition_kind())
            .field("body", &self.body)
            .finish()
    }
}

impl fmt::Debug for PartBody {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PartBody::Leaf(data) => formatter.debug_tuple("Leaf").field(&data.len()).finish(),
            PartBody::Multipart(multipart) => multipart.fmt(formatter),
            PartBody::Message { raw, message } => formatter
                .debug_struct("Message")
                .field("bytes", &raw.len())
                .field("message", message)
                .finish(),
        }
    }
}

impl fmt::Debug for Multipart {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Multipart")
            .field("parts", &self.parts)
            .field("signed", &self.is_signed())
            .field("encrypted", &self.is_encrypted())
            .field("signature", &self.signature())
            .finish()
    }
}

pub(crate) fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n")
}
