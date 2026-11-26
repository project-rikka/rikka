use std::io::{BufRead, Read};

use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD;

use crate::error::{Error, Result};

pub(crate) const VERSION: &[u8] = b"age-encryption.org/v1";
pub(crate) const MAC_BYTES: usize = 32;

const ARMOR: &[u8] = b"-----BEGIN AGE ENCRYPTED FILE-----";
const STANZA: &[u8] = b"-> ";
const FOOTER: &[u8] = b"---";
const COLUMNS: usize = 64;
const MAX_LINE: usize = 1024;
const MAX_HEADER: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Stanza {
    pub(crate) tag: String,
    pub(crate) args: Vec<String>,
    pub(crate) body: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(crate) struct Header {
    pub(crate) stanzas: Vec<Stanza>,
    pub(crate) authenticated: Vec<u8>,
    pub(crate) mac: [u8; MAC_BYTES],
}

impl Stanza {
    pub(crate) fn render(&self, into: &mut Vec<u8>) {
        into.extend_from_slice(STANZA);
        into.extend_from_slice(self.tag.as_bytes());
        for arg in &self.args {
            into.push(b' ');
            into.extend_from_slice(arg.as_bytes());
        }
        into.push(b'\n');
        let body = STANDARD_NO_PAD.encode(&self.body);
        let mut lines = body.as_bytes().chunks(COLUMNS).peekable();
        while let Some(line) = lines.next() {
            into.extend_from_slice(line);
            into.push(b'\n');
            if lines.peek().is_none() && line.len() == COLUMNS {
                into.push(b'\n');
            }
        }
        if body.is_empty() {
            into.push(b'\n');
        }
    }
}

pub(crate) fn authenticated(stanzas: &[Stanza]) -> Vec<u8> {
    let mut header = Vec::new();
    header.extend_from_slice(VERSION);
    header.push(b'\n');
    for stanza in stanzas {
        stanza.render(&mut header);
    }
    header.extend_from_slice(FOOTER);
    header
}

pub(crate) fn render(authenticated: &[u8], mac: &[u8; MAC_BYTES]) -> Vec<u8> {
    let mut header = authenticated.to_vec();
    header.push(b' ');
    header.extend_from_slice(STANDARD_NO_PAD.encode(mac).as_bytes());
    header.push(b'\n');
    header
}

impl Header {
    pub(crate) fn read(input: &mut impl BufRead) -> Result<Header> {
        let mut reader = Lines {
            input,
            consumed: Vec::new(),
        };
        let first = reader.line()?.ok_or(Error::NotAge)?;
        if first.starts_with(ARMOR) {
            return Err(Error::Armored);
        }
        if first != VERSION {
            return Err(Error::NotAge);
        }
        let mut stanzas = Vec::new();
        loop {
            let at = reader.consumed.len();
            let line = reader.line()?.ok_or(Error::Header("missing footer"))?;
            if let Some(args) = line.strip_prefix(STANZA) {
                let (tag, args) = arguments(args)?;
                let body = reader.body()?;
                stanzas.push(Stanza { tag, args, body });
                continue;
            }
            if let Some(mac) = line.strip_prefix(FOOTER) {
                let mac = mac
                    .strip_prefix(b" ")
                    .ok_or(Error::Header("malformed footer"))?;
                let mac: [u8; MAC_BYTES] = decode(mac)?
                    .try_into()
                    .map_err(|_| Error::Header("malformed MAC"))?;
                if stanzas.is_empty() {
                    return Err(Error::Header("no recipient stanza"));
                }
                let mut authenticated = reader.consumed;
                authenticated.truncate(at + FOOTER.len());
                return Ok(Header {
                    stanzas,
                    authenticated,
                    mac,
                });
            }
            return Err(Error::Header("unexpected line"));
        }
    }
}

struct Lines<'a, R> {
    input: &'a mut R,
    consumed: Vec<u8>,
}

impl<R: BufRead> Lines<'_, R> {
    fn line(&mut self) -> Result<Option<Vec<u8>>> {
        let mut line = Vec::new();
        let read =
            Read::take(&mut *self.input, MAX_LINE as u64 + 1).read_until(b'\n', &mut line)?;
        if read == 0 {
            return Ok(None);
        }
        if line.pop() != Some(b'\n') {
            return Err(Error::Header("line too long or unterminated"));
        }
        if self.consumed.len() + read > MAX_HEADER {
            return Err(Error::Header("header too long"));
        }
        self.consumed.extend_from_slice(&line);
        self.consumed.push(b'\n');
        Ok(Some(line))
    }

    fn body(&mut self) -> Result<Vec<u8>> {
        let mut encoded = Vec::new();
        loop {
            let line = self.line()?.ok_or(Error::Header("unterminated stanza"))?;
            if line.len() > COLUMNS {
                return Err(Error::Header("stanza line too long"));
            }
            encoded.extend_from_slice(&line);
            if line.len() < COLUMNS {
                return decode(&encoded);
            }
        }
    }
}

fn arguments(line: &[u8]) -> Result<(String, Vec<String>)> {
    let mut args = Vec::new();
    for arg in line.split(|byte| *byte == b' ') {
        if arg.is_empty() || !arg.iter().all(u8::is_ascii_graphic) {
            return Err(Error::Header("malformed stanza argument"));
        }
        args.push(String::from_utf8_lossy(arg).into_owned());
    }
    let tag = args.remove(0);
    Ok((tag, args))
}

pub(crate) fn decode(encoded: &[u8]) -> Result<Vec<u8>> {
    if encoded.contains(&b'=') {
        return Err(Error::Header("padded base64"));
    }
    STANDARD_NO_PAD
        .decode(encoded)
        .map_err(|_| Error::Header("malformed base64"))
}
