use rikka_audio_decode::Codec;
use rikka_audio_tags::{Audio, Error as TagError, Tagged, Tags};

use crate::artwork::ArtworkCache;
use crate::index::{Record, Stat};

pub(crate) enum Outcome {
    Indexed(Box<Record>),
    Failed(Stat),
}

pub(crate) fn read(stat: Stat, artwork: &ArtworkCache) -> Outcome {
    let tagged = match rikka_audio_tags::read(&stat.path) {
        Ok(tagged) => Some(tagged),
        Err(error) => {
            let codec = match error {
                TagError::Unsupported(codec) => Some(codec),
                _ => None,
            };
            if !error.is_unsupported() {
                tracing::debug!("{}: {error}", stat.path.display());
            }
            probed(&stat, codec)
        }
    };
    let Some(Tagged { mut tags, audio }) = tagged else {
        return Outcome::Failed(stat);
    };
    let cover = tags
        .cover()
        .and_then(|picture| artwork.store(&picture.data));
    tags.pictures.clear();
    Outcome::Indexed(Box::new(Record {
        stat,
        tags,
        audio,
        cover,
    }))
}

fn probed(stat: &Stat, guessed: Option<Codec>) -> Option<Tagged> {
    let info = rikka_audio_decode::read(stat.path.as_path()).probe().ok()?;
    let encoding = info.encoding;
    let codec = match (encoding.codec, guessed) {
        (Codec::Other(_), Some(guessed)) => guessed,
        (probed, _) => probed,
    };
    let rate = match encoding.dsd_rate {
        Some(dsd) => dsd,
        None => info.sample_rate,
    };
    Some(Tagged {
        tags: Tags::default(),
        audio: Audio {
            codec,
            lossless: encoding.lossless,
            duration: info.duration.unwrap_or_default(),
            sample_rate: Some(rate).filter(|rate| *rate > 0),
            bit_depth: info
                .bits_per_sample
                .and_then(|bits| u8::try_from(bits).ok()),
            channels: u8::try_from(info.channels)
                .ok()
                .filter(|channels| *channels > 0),
            bitrate: encoding
                .bitrate
                .map(|bits_per_second| bits_per_second.div_ceil(1000)),
        },
    })
}
