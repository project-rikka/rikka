use std::io::{self, Read, Write};

pub use rikka_types::{CancelToken, Flow};

use crate::error::{Error, Result};

const REPORT_INTERVAL: u64 = 256 * 1024;

#[derive(Debug, Clone, Copy)]
pub struct Progress<'a> {
    pub entry_name: &'a str,
    pub entry_index: usize,
    pub entry_count: usize,
    pub entry_bytes: u64,
    pub entry_size: u64,
    pub total_bytes: u64,
    pub total_size: u64,
}

impl Progress<'_> {
    #[must_use]
    pub fn fraction(&self) -> Option<f32> {
        (self.total_size > 0).then(|| {
            #[expect(
                clippy::cast_precision_loss,
                reason = "a progress bar needs no more precision"
            )]
            let fraction = self.total_bytes as f32 / self.total_size as f32;
            fraction.clamp(0.0, 1.0)
        })
    }
}

pub(crate) type ProgressCallback<'a> = Box<dyn FnMut(Progress<'_>) -> Flow + 'a>;

pub(crate) struct Reporter<'a> {
    callback: Option<ProgressCallback<'a>>,
    cancel: Option<CancelToken>,
    entry_count: usize,
    total_size: u64,
    total_bytes: u64,
    cancelled: bool,
}

impl<'a> Reporter<'a> {
    pub(crate) fn new(
        callback: Option<ProgressCallback<'a>>,
        cancel: Option<CancelToken>,
    ) -> Reporter<'a> {
        Reporter {
            callback,
            cancel,
            entry_count: 0,
            total_size: 0,
            total_bytes: 0,
            cancelled: false,
        }
    }

    pub(crate) fn expect(&mut self, entries: usize, bytes: u64) {
        self.entry_count += entries;
        self.total_size += bytes;
    }

    pub(crate) fn report(
        &mut self,
        entry_name: &str,
        entry_index: usize,
        entry_bytes: u64,
        entry_size: u64,
    ) -> Result<()> {
        if self.cancel.as_ref().is_some_and(CancelToken::is_cancelled) {
            self.cancelled = true;
            return Err(Error::Cancelled);
        }

        let Some(callback) = self.callback.as_mut() else {
            return Ok(());
        };

        let flow = callback(Progress {
            entry_name,
            entry_index,
            entry_count: self.entry_count,
            entry_bytes,
            entry_size,
            total_bytes: self.total_bytes,
            total_size: self.total_size,
        });

        if flow == Flow::Cancel {
            self.cancelled = true;
            return Err(Error::Cancelled);
        }
        Ok(())
    }

    pub(crate) fn advance(&mut self, bytes: u64) {
        self.total_bytes += bytes;
    }

    pub(crate) fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    pub(crate) fn unwrap_cancellation(&self, error: Error) -> Error {
        if self.cancelled {
            Error::Cancelled
        } else {
            error
        }
    }
}

impl std::fmt::Debug for Reporter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reporter")
            .field("entry_count", &self.entry_count)
            .field("total_bytes", &self.total_bytes)
            .field("total_size", &self.total_size)
            .finish_non_exhaustive()
    }
}

pub(crate) struct Metered<'a, 'b, T> {
    inner: T,
    reporter: &'a mut Reporter<'b>,
    entry_name: &'a str,
    entry_index: usize,
    entry_size: u64,
    entry_bytes: u64,
    unreported: u64,
}

impl<'a, 'b, T> Metered<'a, 'b, T> {
    pub(crate) fn new(
        inner: T,
        reporter: &'a mut Reporter<'b>,
        entry_name: &'a str,
        entry_index: usize,
        entry_size: u64,
    ) -> Metered<'a, 'b, T> {
        Metered {
            inner,
            reporter,
            entry_name,
            entry_index,
            entry_size,
            entry_bytes: 0,
            unreported: 0,
        }
    }

    fn record(&mut self, bytes: usize) -> io::Result<()> {
        let bytes = bytes as u64;
        self.entry_bytes += bytes;
        self.unreported += bytes;
        self.reporter.advance(bytes);

        if self.unreported < REPORT_INTERVAL {
            return Ok(());
        }
        self.unreported = 0;
        self.reporter
            .report(
                self.entry_name,
                self.entry_index,
                self.entry_bytes,
                self.entry_size,
            )
            .map_err(|_| io::Error::other("cancelled"))
    }
}

impl<W: Write> Write for Metered<'_, '_, W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buffer)?;
        self.record(written)?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<R: Read> Read for Metered<'_, '_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.record(read)?;
        Ok(read)
    }
}
