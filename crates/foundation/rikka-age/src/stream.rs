use std::io::{self, BufRead, Read, Write};

use zeroize::Zeroizing;

use crate::error::Error;
use crate::keys::{self, SymmetricKey};

pub(crate) const CHUNK: usize = 64 * 1024;

const TAG: usize = 16;
const LAST: u8 = 1;

fn nonce(counter: u64, last: bool) -> io::Result<[u8; 12]> {
    if counter == u64::MAX {
        return Err(io::Error::other("age payload chunk counter overflowed"));
    }
    let mut nonce = [0u8; 12];
    nonce[3..11].copy_from_slice(&counter.to_be_bytes());
    nonce[11] = u8::from(last) * LAST;
    Ok(nonce)
}

pub struct Writer<W: Write> {
    output: W,
    key: SymmetricKey,
    counter: u64,
    pending: Zeroizing<Vec<u8>>,
}

impl<W: Write> Writer<W> {
    pub(crate) fn new(output: W, key: SymmetricKey) -> Writer<W> {
        Writer {
            output,
            key,
            counter: 0,
            pending: Zeroizing::new(Vec::with_capacity(CHUNK + TAG)),
        }
    }

    pub fn finish(mut self) -> io::Result<W> {
        self.seal(true)?;
        self.output.flush()?;
        Ok(self.output)
    }

    fn seal(&mut self, last: bool) -> io::Result<()> {
        let nonce = nonce(self.counter, last)?;
        keys::seal(&self.key, nonce, &mut self.pending)?;
        self.output.write_all(&self.pending)?;
        self.pending.clear();
        self.counter += 1;
        Ok(())
    }
}

impl<W: Write> Write for Writer<W> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if data.is_empty() {
            return Ok(0);
        }
        if self.pending.len() == CHUNK {
            self.seal(false)?;
        }
        let taken = data.len().min(CHUNK - self.pending.len());
        self.pending.extend_from_slice(&data[..taken]);
        Ok(taken)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}

impl<W: Write> std::fmt::Debug for Writer<W> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Writer")
            .field("chunks", &self.counter)
            .finish_non_exhaustive()
    }
}

pub struct Reader<R: BufRead> {
    input: R,
    key: SymmetricKey,
    counter: u64,
    chunk: Zeroizing<Vec<u8>>,
    at: usize,
    finished: bool,
}

impl<R: BufRead> Reader<R> {
    pub(crate) fn new(input: R, key: SymmetricKey) -> Reader<R> {
        Reader {
            input,
            key,
            counter: 0,
            chunk: Zeroizing::new(Vec::with_capacity(CHUNK + TAG)),
            at: 0,
            finished: false,
        }
    }

    fn next_chunk(&mut self) -> io::Result<()> {
        self.chunk.clear();
        self.at = 0;
        Read::take(&mut self.input, (CHUNK + TAG) as u64).read_to_end(&mut self.chunk)?;
        if self.chunk.len() < TAG {
            return Err(Error::Truncated.into());
        }
        let last = self.chunk.len() < CHUNK + TAG || self.input.fill_buf()?.is_empty();
        if !keys::open(&self.key, nonce(self.counter, last)?, &mut self.chunk) {
            return Err(Error::Payload.into());
        }
        if last && self.chunk.is_empty() && self.counter > 0 {
            return Err(Error::Payload.into());
        }
        self.counter += 1;
        self.finished = last;
        Ok(())
    }
}

impl<R: BufRead> Read for Reader<R> {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        while self.at == self.chunk.len() {
            if self.finished {
                return Ok(0);
            }
            self.next_chunk()?;
        }
        let taken = into.len().min(self.chunk.len() - self.at);
        into[..taken].copy_from_slice(&self.chunk[self.at..self.at + taken]);
        self.at += taken;
        Ok(taken)
    }
}

impl<R: BufRead> std::fmt::Debug for Reader<R> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Reader")
            .field("chunks", &self.counter)
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}
