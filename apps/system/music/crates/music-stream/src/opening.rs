use std::time::Duration;

use music_core::Fidelity;
use rikka_audio_decode::Decoder;
use rikka_hifi::{Gains, Source};

use crate::origin::{Located, Origin};
use crate::spool::Stored;

pub(crate) enum Opening {
    Located(Located),
    Stored(Stored),
}

pub(crate) struct Ready {
    pub(crate) source: Source,
    pub(crate) length: Option<Duration>,
    pub(crate) fidelity: Fidelity,
}

impl Opening {
    pub(crate) fn is_streamed(&self) -> bool {
        matches!(self, Opening::Stored(_))
    }

    pub(crate) fn ready(self, origin: &dyn Origin, at: Duration) -> Result<Ready, String> {
        let (source, length) = match self {
            Opening::Located(located) => {
                let source = Source::file(&located.path, at).map_err(|error| error.to_string())?;
                let length = source.length();
                (source.gains(located.gains), length)
            }
            Opening::Stored(stored) => {
                let reader = origin.open(stored).map_err(|error| error.to_string())?;
                let decoder = rikka_audio_decode::reader_decoder(reader)
                    .map_err(|error| error.to_string())?;
                let decoder = from(decoder, at)?;
                let length = decoder.duration();
                (Source::decoder(decoder), length)
            }
        };
        Ok(Ready {
            fidelity: Fidelity::from_source_format(source.format()),
            source,
            length,
        })
    }
}

impl Ready {
    pub(crate) fn gained(self, gains: Gains) -> Ready {
        Ready {
            source: self.source.gains(gains),
            ..self
        }
    }
}

fn from(mut decoder: Decoder, at: Duration) -> Result<Decoder, String> {
    if !at.is_zero() {
        decoder.seek(at).map_err(|error| error.to_string())?;
    }
    Ok(decoder)
}
