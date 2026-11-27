use std::fmt::Write;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use jiff::Zoned;
use rand::RngCore;

use crate::address::{Address, header_list};
use crate::error::{Error, Result};
use crate::words;

const LINE: usize = 76;
const CRLF: &str = "\r\n";
const BINARY: &str = "application/octet-stream";

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Enclosure {
    pub name: String,
    pub mime: String,
    pub content: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outgoing {
    pub from: Address,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    pub bcc: Vec<Address>,
    pub subject: String,
    pub text: String,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    pub enclosures: Vec<Enclosure>,
}

impl Outgoing {
    #[must_use]
    pub fn has_recipient(&self) -> bool {
        !(self.to.is_empty() && self.cc.is_empty() && self.bcc.is_empty())
    }

    pub fn to_rfc822(&self) -> Result<Vec<u8>> {
        self.rfc822(true)
    }

    pub(crate) fn rfc822(&self, deliverable: bool) -> Result<Vec<u8>> {
        if deliverable && !self.has_recipient() {
            return Err(Error::NoRecipient);
        }
        for address in self.to.iter().chain(&self.cc).chain(&self.bcc) {
            if !address.is_deliverable() {
                return Err(Error::Address(address.email.clone()));
            }
        }
        let mut message = String::new();
        self.head(&mut message).expect("strings accept text");
        let mut message = message.into_bytes();
        if self.enclosures.is_empty() {
            text_part(&mut message, &self.text);
            return Ok(message);
        }
        let boundary = boundary();
        let line = |message: &mut Vec<u8>, text: &str| {
            message.extend_from_slice(text.as_bytes());
            message.extend_from_slice(CRLF.as_bytes());
        };
        line(
            &mut message,
            &format!("Content-Type: multipart/mixed; boundary=\"{boundary}\""),
        );
        line(&mut message, "");
        line(&mut message, &format!("--{boundary}"));
        text_part(&mut message, &self.text);
        for enclosure in &self.enclosures {
            line(&mut message, &format!("--{boundary}"));
            enclosure.part(&mut message);
        }
        line(&mut message, &format!("--{boundary}--"));
        Ok(message)
    }

    fn head(&self, out: &mut String) -> std::fmt::Result {
        write!(out, "From: {}{CRLF}", self.from.header())?;
        for (name, addresses) in [("To", &self.to), ("Cc", &self.cc), ("Bcc", &self.bcc)] {
            if !addresses.is_empty() {
                write!(out, "{name}: {}{CRLF}", header_list(addresses))?;
            }
        }
        write!(
            out,
            "Subject: {}{CRLF}",
            words::unstructured(&self.subject, "Subject: ".len())
        )?;
        write!(
            out,
            "Date: {}{CRLF}",
            Zoned::now().strftime("%a, %d %b %Y %H:%M:%S %z")
        )?;
        if let Some(parent) = self.in_reply_to.as_deref().filter(|id| !id.is_empty()) {
            write!(out, "In-Reply-To: {parent}{CRLF}")?;
        }
        if !self.references.is_empty() {
            write!(out, "References: {}{CRLF}", self.references.join("\r\n "))?;
        }
        write!(out, "MIME-Version: 1.0{CRLF}")
    }
}

impl Enclosure {
    fn part(&self, message: &mut Vec<u8>) {
        let mime = match self.mime.is_empty() {
            true => BINARY,
            false => &self.mime,
        };
        let head = format!(
            "Content-Type: {mime}{CRLF}Content-Disposition: attachment; {}{CRLF}\
             Content-Transfer-Encoding: base64{CRLF}{CRLF}",
            file_name(&self.name)
        );
        message.extend_from_slice(head.as_bytes());
        wrapped(message, &self.content);
    }
}

impl std::fmt::Debug for Enclosure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Enclosure")
            .field("name", &self.name)
            .field("bytes", &self.content.len())
            .finish_non_exhaustive()
    }
}

fn text_part(message: &mut Vec<u8>, text: &str) {
    let head = format!(
        "Content-Type: text/plain; charset=UTF-8{CRLF}Content-Transfer-Encoding: base64{CRLF}{CRLF}"
    );
    message.extend_from_slice(head.as_bytes());
    let normalized = text.replace("\r\n", "\n").replace('\n', CRLF);
    wrapped(message, normalized.as_bytes());
}

fn wrapped(message: &mut Vec<u8>, content: &[u8]) {
    let encoded = STANDARD.encode(content);
    for line in encoded.as_bytes().chunks(LINE) {
        message.extend_from_slice(line);
        message.extend_from_slice(CRLF.as_bytes());
    }
}

fn file_name(name: &str) -> String {
    let plain = name
        .bytes()
        .all(|byte| byte.is_ascii_graphic() && !matches!(byte, b'"' | b'\\') || byte == b' ');
    if plain {
        return format!("filename=\"{name}\"");
    }
    let mut encoded = String::from("filename*=UTF-8''");
    for byte in name.bytes() {
        match byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_') {
            true => encoded.push(char::from(byte)),
            false => write!(encoded, "%{byte:02X}").expect("strings accept text"),
        }
    }
    encoded
}

fn boundary() -> String {
    let mut random = [0; 16];
    rand::rng().fill_bytes(&mut random);
    format!("rikka-{}", rikka_types::hex::encode(random))
}
