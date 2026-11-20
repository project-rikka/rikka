use std::sync::Arc;
use std::time::Duration;

use rikka_hifi_core::{Gains, SourceFormat};
use rikka_worker::{EventNotifier, Worker};

use crate::source::feed::{Closing, Feed, ReadStatus};
use crate::source::producing::spawn;
use crate::source::{Origin, Source, SourceId, ahead};

enum Samples {
    Pcm(Arc<Feed<f32>>),
    Dsd(Arc<Feed<u8>>),
}

pub(crate) struct SourceStream {
    pub(crate) id: SourceId,
    pub(crate) format: SourceFormat,
    pub(crate) gains: Gains,
    samples: Samples,
    _producer: Option<Worker<Closing>>,
}

impl SourceStream {
    pub(crate) fn open(
        id: SourceId,
        source: Source,
        wake: &EventNotifier,
    ) -> Result<SourceStream, Unstarted> {
        let Source {
            format,
            gains,
            origin,
        } = source;
        let (samples, producer) = start(origin, &format, wake)?;
        Ok(SourceStream {
            id,
            format,
            gains,
            samples,
            _producer: producer,
        })
    }

    pub(crate) fn is_seekable(&self) -> bool {
        match &self.samples {
            Samples::Pcm(feed) => feed.is_seekable(),
            Samples::Dsd(feed) => feed.is_seekable(),
        }
    }

    pub(crate) fn seek(&self, to: Duration) {
        match &self.samples {
            Samples::Pcm(feed) => feed.seek(to),
            Samples::Dsd(feed) => feed.seek(to),
        }
    }

    pub(crate) fn take(&self, pcm: &mut Vec<f32>, dsd: &mut Vec<u8>, frames: usize) -> ReadStatus {
        let channels = usize::from(self.format.channels);
        match &self.samples {
            Samples::Pcm(feed) => feed.take(pcm, frames * channels),
            Samples::Dsd(feed) => feed.take(dsd, frames * channels),
        }
    }

    fn close(&self) {
        match &self.samples {
            Samples::Pcm(feed) => feed.close(),
            Samples::Dsd(feed) => feed.close(),
        }
    }
}

type Started = (Samples, Option<Worker<Closing>>);

pub(crate) type Unstarted = rikka_worker::Error<std::convert::Infallible>;

fn start(
    origin: Origin,
    format: &SourceFormat,
    wake: &EventNotifier,
) -> Result<Started, Unstarted> {
    let frame = usize::from(format.channels);
    let (samples, producer) = match origin {
        Origin::Decoder(decoder) => {
            let feed = Feed::new(ahead(format), frame, true);
            let worker = spawn(decoder, &feed)?;
            (Samples::Pcm(feed), Some(worker))
        }
        Origin::Dsd(reader) => {
            let feed = Feed::new(ahead(format), frame, true);
            let worker = spawn(reader, &feed)?;
            (Samples::Dsd(feed), Some(worker))
        }
        Origin::Pushed(feed) => (Samples::Pcm(feed), None),
    };
    match &samples {
        Samples::Pcm(feed) => feed.attach(wake.clone()),
        Samples::Dsd(feed) => feed.attach(wake.clone()),
    }
    Ok((samples, producer))
}

impl Drop for SourceStream {
    fn drop(&mut self) {
        self.close();
    }
}

impl std::fmt::Debug for SourceStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceStream")
            .field("id", &self.id)
            .field("format", &self.format)
            .finish_non_exhaustive()
    }
}
