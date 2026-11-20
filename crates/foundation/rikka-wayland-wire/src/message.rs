use std::cell::RefMut;
use std::collections::VecDeque;
use std::fmt;
use std::os::fd::OwnedFd;

use crate::error::{Result, WireError};
use crate::incoming::Incoming;
use crate::interface::{ArgType, Signature};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub object: u32,
    pub opcode: u16,
    pub size: usize,
}

pub struct Message<'c> {
    header: Header,
    incoming: RefMut<'c, Incoming>,
}

impl<'c> Message<'c> {
    pub(crate) fn new(header: Header, incoming: RefMut<'c, Incoming>) -> Self {
        Self { header, incoming }
    }

    pub fn header(&self) -> Header {
        self.header
    }

    pub fn object(&self) -> u32 {
        self.header.object
    }

    pub fn opcode(&self) -> u16 {
        self.header.opcode
    }

    pub fn body(&self) -> &[u8] {
        self.incoming.body(self.header.size)
    }

    pub(crate) fn take_fds(&mut self, count: usize) -> Result<VecDeque<OwnedFd>> {
        let (_, fds) = self.incoming.message(self.header.size);
        if fds.len() < count {
            return Err(WireError::MissingFd);
        }
        Ok(fds.drain(..count).collect())
    }

    pub fn args(&mut self, signature: Signature) -> Args<'_> {
        let (body, fds) = self.incoming.message(self.header.size);
        Args {
            body,
            offset: 0,
            signature,
            index: 0,
            fds,
        }
    }
}

impl Drop for Message<'_> {
    fn drop(&mut self) {
        self.incoming.consume(self.header.size);
    }
}

impl fmt::Debug for Message<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Message")
            .field("header", &self.header)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum Arg<'m> {
    Int(i32),
    Uint(u32),
    Fixed(f64),
    String(Option<&'m str>),
    Object(Option<u32>),
    NewId(u32),
    UntypedNewId(NewId<'m>),
    Array(&'m [u8]),
    Fd(OwnedFd),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewId<'m> {
    pub interface: &'m str,
    pub version: u32,
    pub id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Word(u32);

impl Word {
    pub fn int(self) -> i32 {
        self.0 as i32
    }

    pub fn uint(self) -> u32 {
        self.0
    }

    pub fn fixed(self) -> f64 {
        f64::from(self.0 as i32) / 256.0
    }

    pub fn object(self) -> Result<u32> {
        self.required("object")
    }

    pub fn nullable_object(self) -> Option<u32> {
        Some(self.0).filter(|id| *id != 0)
    }

    pub fn new_id(self) -> Result<u32> {
        self.required("new_id")
    }

    fn required(self, kind: &'static str) -> Result<u32> {
        match self.0 {
            0 => Err(WireError::Null(kind)),
            id => Ok(id),
        }
    }
}

pub struct Args<'m> {
    body: &'m [u8],
    offset: usize,
    signature: Signature,
    index: usize,
    fds: &'m mut VecDeque<OwnedFd>,
}

impl fmt::Debug for Args<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Args")
            .field("signature", &self.signature)
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

impl<'m> Args<'m> {
    pub(crate) fn new(
        body: &'m [u8],
        signature: Signature,
        fds: &'m mut VecDeque<OwnedFd>,
    ) -> Self {
        Self {
            body,
            offset: 0,
            signature,
            index: 0,
            fds,
        }
    }

    pub fn int(&mut self) -> Result<i32> {
        self.expect(ArgType::Int)?;
        Ok(self.word()?.int())
    }

    pub fn uint(&mut self) -> Result<u32> {
        self.expect(ArgType::Uint)?;
        Ok(self.word()?.uint())
    }

    pub fn fixed(&mut self) -> Result<f64> {
        self.expect(ArgType::Fixed)?;
        Ok(self.word()?.fixed())
    }

    pub fn string(&mut self) -> Result<&'m str> {
        self.expect(ArgType::String)?;
        self.text()?.ok_or(WireError::Null("string"))
    }

    pub fn nullable_string(&mut self) -> Result<Option<&'m str>> {
        self.expect(ArgType::NullableString)?;
        self.text()
    }

    pub fn object(&mut self) -> Result<u32> {
        self.expect(ArgType::Object)?;
        self.word()?.object()
    }

    pub fn nullable_object(&mut self) -> Result<Option<u32>> {
        self.expect(ArgType::NullableObject)?;
        Ok(self.word()?.nullable_object())
    }

    pub fn new_id(&mut self) -> Result<u32> {
        self.expect(ArgType::NewId)?;
        self.word()?.new_id()
    }

    pub fn untyped_new_id(&mut self) -> Result<NewId<'m>> {
        self.expect(ArgType::UntypedNewId)?;
        self.untyped()
    }

    pub fn array(&mut self) -> Result<&'m [u8]> {
        self.expect(ArgType::Array)?;
        self.array_bytes()
    }

    pub fn fd(&mut self) -> Result<OwnedFd> {
        self.expect(ArgType::Fd)?;
        self.fds.pop_front().ok_or(WireError::MissingFd)
    }

    pub fn words<const N: usize>(&mut self) -> Result<[Word; N]> {
        if cfg!(debug_assertions) {
            self.expect_words(N)?;
        } else {
            self.index += N;
        }
        let end = self.offset + 4 * N;
        let bytes = self
            .body
            .get(self.offset..end)
            .ok_or(WireError::Truncated)?;
        self.offset = end;
        let mut words = [Word(0); N];
        let (chunks, _) = bytes.as_chunks::<4>();
        for (word, chunk) in words.iter_mut().zip(chunks) {
            *word = Word(u32::from_ne_bytes(*chunk));
        }
        Ok(words)
    }

    pub fn finish(self) -> Result<()> {
        if let Some(expected) = self.signature.get(self.index) {
            return Err(WireError::Signature {
                index: self.index,
                read: "the end",
                expected: expected.name(),
            });
        }
        match self.body.len() - self.offset {
            0 => Ok(()),
            trailing => Err(WireError::TrailingBytes(trailing)),
        }
    }

    #[inline]
    fn expect(&mut self, read: ArgType) -> Result<()> {
        if cfg!(debug_assertions) && self.signature.get(self.index) != Some(&read) {
            return Err(self.mismatch(read.name()));
        }
        self.index += 1;
        Ok(())
    }

    fn expect_words(&mut self, count: usize) -> Result<()> {
        for _ in 0..count {
            let is_word = self.signature.get(self.index).is_some_and(|kind| {
                matches!(
                    kind,
                    ArgType::Int
                        | ArgType::Uint
                        | ArgType::Fixed
                        | ArgType::Object
                        | ArgType::NullableObject
                        | ArgType::NewId
                )
            });
            if !is_word {
                return Err(self.mismatch("a word"));
            }
            self.index += 1;
        }
        Ok(())
    }

    #[cold]
    fn mismatch(&self, read: &'static str) -> WireError {
        WireError::Signature {
            index: self.index,
            read,
            expected: self
                .signature
                .get(self.index)
                .map_or("the end", |expected| expected.name()),
        }
    }

    fn word(&mut self) -> Result<Word> {
        let end = self.offset + 4;
        let bytes = self
            .body
            .get(self.offset..end)
            .ok_or(WireError::Truncated)?;
        self.offset = end;
        Ok(Word(u32::from_ne_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
        ])))
    }

    fn untyped(&mut self) -> Result<NewId<'m>> {
        let interface = self.text()?.ok_or(WireError::Null("string"))?;
        let version = self.word()?.uint();
        let id = self.word()?.new_id()?;
        Ok(NewId {
            interface,
            version,
            id,
        })
    }

    fn array_bytes(&mut self) -> Result<&'m [u8]> {
        let length = self.word()?.uint() as usize;
        self.bytes(length)
    }

    fn bytes(&mut self, length: usize) -> Result<&'m [u8]> {
        let padded = length.checked_add(3).ok_or(WireError::Truncated)? & !3;
        let end = self
            .offset
            .checked_add(padded)
            .ok_or(WireError::Truncated)?;
        let body: &'m [u8] = self.body;
        let bytes = body.get(self.offset..end).ok_or(WireError::Truncated)?;
        self.offset = end;
        Ok(&bytes[..length])
    }

    fn text(&mut self) -> Result<Option<&'m str>> {
        let length = self.word()?.uint() as usize;
        if length == 0 {
            return Ok(None);
        }
        let bytes = self.bytes(length)?;
        let (terminator, content) = bytes.split_last().ok_or(WireError::Unterminated)?;
        if *terminator != 0 {
            return Err(WireError::Unterminated);
        }
        let content = match content.iter().position(|byte| *byte == 0) {
            Some(end) => &content[..end],
            None => content,
        };
        std::str::from_utf8(content)
            .map(Some)
            .map_err(|_| WireError::NotUtf8)
    }

    fn decode(&mut self, kind: ArgType) -> Result<Arg<'m>> {
        Ok(match kind {
            ArgType::Int => Arg::Int(self.word()?.int()),
            ArgType::Uint => Arg::Uint(self.word()?.uint()),
            ArgType::Fixed => Arg::Fixed(self.word()?.fixed()),
            ArgType::String => Arg::String(Some(self.text()?.ok_or(WireError::Null("string"))?)),
            ArgType::NullableString => Arg::String(self.text()?),
            ArgType::Object => Arg::Object(Some(self.word()?.object()?)),
            ArgType::NullableObject => Arg::Object(self.word()?.nullable_object()),
            ArgType::NewId => Arg::NewId(self.word()?.new_id()?),
            ArgType::UntypedNewId => Arg::UntypedNewId(self.untyped()?),
            ArgType::Array => Arg::Array(self.array_bytes()?),
            ArgType::Fd => Arg::Fd(self.fds.pop_front().ok_or(WireError::MissingFd)?),
        })
    }
}

impl<'m> Iterator for Args<'m> {
    type Item = Result<Arg<'m>>;

    fn next(&mut self) -> Option<Self::Item> {
        let kind = *self.signature.get(self.index)?;
        self.index += 1;
        Some(self.decode(kind))
    }
}
