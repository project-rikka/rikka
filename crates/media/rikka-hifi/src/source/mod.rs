mod feed;
mod file;
mod producing;
mod stream;
mod writer;

use std::sync::Arc;
use std::time::Duration;

use rikka_audio_decode::{Decoder, DsdReader};
use rikka_hifi_core::{Gains, SourceFormat};

pub(crate) use feed::{Feed, ReadStatus};
pub(crate) use stream::SourceStream;
pub use writer::PcmWriter;

const AHEAD_SECONDS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceId(pub(crate) u64);

pub struct Source {
    pub(crate) format: SourceFormat,
    pub(crate) gains: Gains,
    pub(crate) origin: Origin,
}

pub(crate) enum Origin {
    Decoder(Decoder),
    Dsd(DsdReader),
    Pushed(Arc<Feed<f32>>),
}

impl Source {
    #[must_use]
    pub fn decoder(decoder: Decoder) -> Source {
        Source {
            format: SourceFormat::from(decoder.info()),
            gains: Gains::default(),
            origin: Origin::Decoder(decoder),
        }
    }

    #[must_use]
    pub fn dsd(reader: DsdReader) -> Source {
        Source {
            format: SourceFormat::dsd(reader.rate(), reader.channels()),
            gains: Gains::default(),
            origin: Origin::Dsd(reader),
        }
    }

    #[must_use]
    pub fn pushed(format: SourceFormat) -> (Source, PcmWriter) {
        let frame = usize::from(format.channels);
        let feed = Feed::new(ahead(&format), frame, false);
        let writer = PcmWriter::new(Arc::clone(&feed));
        let source = Source {
            format,
            gains: Gains::default(),
            origin: Origin::Pushed(feed),
        };
        (source, writer)
    }

    #[must_use]
    pub fn gains(mut self, gains: Gains) -> Source {
        self.gains = gains;
        self
    }

    #[must_use]
    pub fn format(&self) -> &SourceFormat {
        &self.format
    }

    #[must_use]
    pub fn length(&self) -> Option<Duration> {
        match &self.origin {
            Origin::Decoder(decoder) => decoder.duration(),
            Origin::Dsd(reader) => Some(reader.duration()),
            Origin::Pushed(_) => None,
        }
    }
}

pub(crate) fn ahead(format: &SourceFormat) -> usize {
    let per_second = if format.is_dsd() {
        format.rate() as usize / 8
    } else {
        format.rate() as usize
    };
    per_second * usize::from(format.channels) * AHEAD_SECONDS
}

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let origin = match self.origin {
            Origin::Decoder(_) => "decoder",
            Origin::Dsd(_) => "dsd",
            Origin::Pushed(_) => "pushed",
        };
        f.debug_struct("Source")
            .field("format", &self.format)
            .field("gains", &self.gains)
            .field("origin", &origin)
            .finish()
    }
}
