use crate::bytes::Bytes;
use crate::error::{Error, Result};
use crate::fetch::Fetch;
use crate::format::Format;

const HEADER: u64 = 8;
const LARGE_HEADER: u64 = 16;
const UUID: [u8; 4] = *b"uuid";
const UUID_BYTES: usize = 16;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Atom<'d> {
    pub(crate) kind: [u8; 4],
    pub(crate) body: &'d [u8],
}

impl<'d> Atom<'d> {
    pub(crate) fn full(&self, format: Format) -> Result<(u8, u32, Bytes<'d>)> {
        let mut bytes = Bytes::new(format, self.body);
        let header = bytes.u32_be()?;
        Ok(((header >> 24) as u8, header & 0x00FF_FFFF, bytes))
    }
}

pub(crate) struct Atoms<'d> {
    bytes: Bytes<'d>,
    format: Format,
}

impl<'d> Atoms<'d> {
    pub(crate) fn new(format: Format, data: &'d [u8]) -> Atoms<'d> {
        Atoms {
            bytes: Bytes::new(format, data),
            format,
        }
    }

    pub(crate) fn find(format: Format, data: &'d [u8], kind: [u8; 4]) -> Result<Option<Atom<'d>>> {
        for atom in Atoms::new(format, data) {
            let atom = atom?;
            if atom.kind == kind {
                return Ok(Some(atom));
            }
        }
        Ok(None)
    }

    fn read(&mut self) -> Result<Atom<'d>> {
        let size = u64::from(self.bytes.u32_be()?);
        let kind = self.bytes.fourcc()?;
        let (header, size) = match size {
            0 => (HEADER, HEADER + self.bytes.remaining() as u64),
            1 => (LARGE_HEADER, self.bytes.u64_be()?),
            size => (HEADER, size),
        };
        let extended = match kind == UUID {
            true => {
                self.bytes.skip(UUID_BYTES)?;
                UUID_BYTES as u64
            }
            false => 0,
        };
        let body = size
            .checked_sub(header + extended)
            .ok_or_else(|| Error::malformed(self.format, "box size"))?;
        let body = usize::try_from(body).map_err(|_| self.bytes.short())?;
        Ok(Atom {
            kind,
            body: self.bytes.take(body)?,
        })
    }
}

impl<'d> Iterator for Atoms<'d> {
    type Item = Result<Atom<'d>>;

    fn next(&mut self) -> Option<Result<Atom<'d>>> {
        if self.bytes.is_empty() {
            return None;
        }
        let atom = self.read();
        if atom.is_err() {
            self.bytes.exhaust();
        }
        Some(atom)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Span {
    pub(crate) kind: [u8; 4],
    pub(crate) at: u64,
    pub(crate) len: u64,
}

pub(crate) fn top_level(fetch: &mut Fetch<'_>) -> Result<Vec<Span>> {
    let format = fetch.format();
    let mut spans = Vec::new();
    let mut at = 0;
    while at < fetch.len() {
        let head = fetch.fetch_up_to(at, LARGE_HEADER)?;
        let mut bytes = Bytes::new(format, &head);
        let size = u64::from(bytes.u32_be()?);
        let kind = bytes.fourcc()?;
        let (header, size) = match size {
            0 => (HEADER, fetch.len() - at),
            1 => (LARGE_HEADER, bytes.u64_be()?),
            size => (HEADER, size),
        };
        let len = size
            .checked_sub(header)
            .ok_or_else(|| Error::malformed(format, "box size"))?;
        let end = at
            .checked_add(size)
            .filter(|&end| end <= fetch.len())
            .ok_or(Error::Truncated(format))?;
        spans.push(Span {
            kind,
            at: at + header,
            len,
        });
        at = end;
    }
    Ok(spans)
}
