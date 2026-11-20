use std::io::{self, Read, Seek, SeekFrom};

use music_core::{Error, Result};
use music_qobuz_keys::{ContentKey, Vault, decrypt_frame};
use music_stream::Stored;
use rikka_audio_decode::StreamReader;
use rikka_file_format::{BinaryFormat, Reader};

use crate::segments::{self, Piece};

const FORMAT: BinaryFormat = BinaryFormat::new("sealed Qobuz stream", *b"RKQOBUZS");

pub(crate) fn head(sealed_key: &[u8], init: &[u8]) -> Result<Vec<u8>> {
    let key_len = u16::try_from(sealed_key.len()).map_err(|_| too_large())?;
    let mut head = FORMAT.header().to_vec();
    head.extend_from_slice(&key_len.to_be_bytes());
    head.extend_from_slice(sealed_key);
    head.extend_from_slice(&record_len(init.len() as u64)?);
    head.extend_from_slice(init);
    Ok(head)
}

pub(crate) fn record_len(len: u64) -> Result<[u8; 4]> {
    u32::try_from(len)
        .map(u32::to_be_bytes)
        .map_err(|_| too_large())
}

pub(crate) struct Decrypted {
    stored: Stored,
    key: ContentKey,
    header: Vec<u8>,
    lengths: Vec<u64>,
    starts: Vec<u64>,
    total: u64,
    records: Vec<(u64, u64)>,
    next_record: u64,
    layout: Option<Layout>,
    frame: Option<(usize, usize, Vec<u8>)>,
    position: u64,
}

struct Layout {
    segment: usize,
    pieces: Vec<Piece>,
    starts: Vec<u64>,
}

impl Decrypted {
    pub(crate) fn open(mut stored: Stored, vault: &Vault) -> Result<Decrypted> {
        let mut header = [0; BinaryFormat::HEADER_LEN];
        read_at(&mut stored, 0, &mut header).map_err(damaged)?;
        FORMAT
            .read_header(&mut Reader::new(&header))
            .map_err(|error| Error::Unreadable(format!("cached track: {error}")))?;
        let mut at = BinaryFormat::HEADER_LEN as u64;
        let key_len = u64::from(u16::from_be_bytes(take(&mut stored, &mut at)?));
        let sealed = take_vec(&mut stored, &mut at, key_len)?;
        let key = vault.unseal(&sealed)?;
        let init_len = u64::from(u32::from_be_bytes(take(&mut stored, &mut at)?));
        let init = segments::init(&take_vec(&mut stored, &mut at, init_len)?)?;
        let mut starts = Vec::with_capacity(init.lengths.len());
        let mut total = init.header.len() as u64;
        for length in &init.lengths {
            starts.push(total);
            total += length;
        }
        Ok(Decrypted {
            stored,
            key,
            header: init.header,
            lengths: init.lengths,
            starts,
            total,
            records: Vec::new(),
            next_record: at,
            layout: None,
            frame: None,
            position: 0,
        })
    }

    fn record(&mut self, segment: usize) -> io::Result<(u64, u64)> {
        while self.records.len() <= segment {
            let mut len = [0; 4];
            read_at(&mut self.stored, self.next_record, &mut len)?;
            let len = u64::from(u32::from_be_bytes(len));
            let start = self.next_record + 4;
            self.records.push((start, len));
            self.next_record = start + len;
        }
        Ok(self.records[segment])
    }

    fn layout(&mut self, segment: usize) -> io::Result<&Layout> {
        if self
            .layout
            .as_ref()
            .is_none_or(|current| current.segment != segment)
        {
            let (start, len) = self.record(segment)?;
            let stored = &mut self.stored;
            let pieces = segments::pieces(len, &mut |at, into| read_at(stored, start + at, into))
                .map_err(io::Error::other)?;
            let mut starts = Vec::with_capacity(pieces.len());
            let mut size = 0;
            for piece in &pieces {
                starts.push(size);
                size += piece.len;
            }
            if size != self.lengths[segment] {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "segment {} holds {size} bytes of audio, not {}",
                        segment + 1,
                        self.lengths[segment]
                    ),
                ));
            }
            let pieces = pieces
                .into_iter()
                .map(|piece| Piece {
                    at: start + piece.at,
                    ..piece
                })
                .collect();
            self.layout = Some(Layout {
                segment,
                pieces,
                starts,
            });
        }
        Ok(self.layout.as_ref().expect("layout computed above"))
    }

    fn frame(&mut self, segment: usize, within: u64) -> io::Result<(u64, &[u8])> {
        let layout = self.layout(segment)?;
        let index = layout.starts.partition_point(|start| *start <= within) - 1;
        let piece = layout.pieces[index];
        let piece_start = layout.starts[index];
        let present = self
            .frame
            .as_ref()
            .is_some_and(|(held_segment, held_index, _)| {
                *held_segment == segment && *held_index == index
            });
        if !present {
            let mut bytes = vec![0; usize::try_from(piece.len).map_err(io::Error::other)?];
            read_at(&mut self.stored, piece.at, &mut bytes)?;
            if let Some(iv) = piece.iv {
                decrypt_frame(&self.key, &iv, &mut bytes);
            }
            self.frame = Some((segment, index, bytes));
        }
        let (_, _, bytes) = self.frame.as_ref().expect("frame present after read");
        Ok((piece_start, bytes))
    }
}

impl Read for Decrypted {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let position = self.position;
        if buf.is_empty() || position >= self.total {
            return Ok(0);
        }
        let header = self.header.len() as u64;
        let copied = if position < header {
            let from = &self.header[position as usize..];
            let count = from.len().min(buf.len());
            buf[..count].copy_from_slice(&from[..count]);
            count
        } else {
            let segment = self.starts.partition_point(|start| *start <= position) - 1;
            let within = position - self.starts[segment];
            let (piece_start, bytes) = self.frame(segment, within)?;
            let from = &bytes[(within - piece_start) as usize..];
            let count = from.len().min(buf.len());
            buf[..count].copy_from_slice(&from[..count]);
            count
        };
        self.position += copied as u64;
        Ok(copied)
    }
}

impl Seek for Decrypted {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let target = match to {
            SeekFrom::Start(offset) => Some(offset),
            SeekFrom::Current(delta) => self.position.checked_add_signed(delta),
            SeekFrom::End(delta) => self.total.checked_add_signed(delta),
        };
        self.position = target.ok_or_else(|| io::Error::other("a seek before the start"))?;
        Ok(self.position)
    }
}

impl StreamReader for Decrypted {
    fn complete_len(&self) -> Option<u64> {
        self.stored.complete_len().map(|_| self.total)
    }
}

fn read_at(stored: &mut Stored, at: u64, into: &mut [u8]) -> io::Result<()> {
    stored.seek(SeekFrom::Start(at))?;
    stored.read_exact(into)
}

fn take<const N: usize>(stored: &mut Stored, at: &mut u64) -> Result<[u8; N]> {
    let mut bytes = [0; N];
    read_at(stored, *at, &mut bytes).map_err(damaged)?;
    *at += N as u64;
    Ok(bytes)
}

fn take_vec(stored: &mut Stored, at: &mut u64, len: u64) -> Result<Vec<u8>> {
    let mut bytes = vec![0; usize::try_from(len).map_err(|_| too_large())?];
    read_at(stored, *at, &mut bytes).map_err(damaged)?;
    *at += len;
    Ok(bytes)
}

fn damaged(error: io::Error) -> Error {
    Error::Unreadable(format!("cannot read the cached track: {error}"))
}

fn too_large() -> Error {
    Error::Unreadable("a segment is too large".into())
}
