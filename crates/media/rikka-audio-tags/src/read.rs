use std::fs::File;
use std::io::Read;
use std::path::Path;

use lofty::probe::Probe;

use crate::audio::Audio;
use crate::container;
use crate::dsd::{self, Stream};
use crate::error::{Error, Result};
use crate::id3;
use crate::normalize;
use crate::tags::Tags;
use rikka_audio_decode::Codec;

const SNIFF_LEN: usize = 16;
const ASF: [u8; 16] = [
    0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE, 0x6C,
];

#[derive(Clone, Debug, PartialEq)]
pub struct Tagged {
    pub tags: Tags,
    pub audio: Audio,
}

enum ContainerKind {
    Dsd,
    Unsupported(Codec),
    Other,
}

pub fn read(path: impl AsRef<Path>) -> Result<Tagged> {
    let path = path.as_ref();
    let mut head = Vec::with_capacity(SNIFF_LEN);
    File::open(path)?
        .take(SNIFF_LEN as u64)
        .read_to_end(&mut head)?;
    match sniff(&head) {
        ContainerKind::Dsd => Ok(streamed(dsd::read(path)?)),
        ContainerKind::Unsupported(codec) => Err(Error::Unsupported(codec)),
        ContainerKind::Other => {
            let probe = Probe::open(path)?.guess_file_type()?;
            let file_type = probe.file_type().ok_or(Error::Unknown)?;
            let container = container::read(&mut probe.into_inner(), file_type)?;
            Ok(Tagged {
                tags: normalize::tags(container.tags),
                audio: container.audio,
            })
        }
    }
}

fn streamed(stream: Stream) -> Tagged {
    Tagged {
        tags: normalize::tags(stream.id3.map(id3::tags).unwrap_or_default()),
        audio: stream.audio,
    }
}

fn sniff(head: &[u8]) -> ContainerKind {
    match head {
        [b'D', b'S', b'D', b' ', ..] => ContainerKind::Dsd,
        [
            b'F',
            b'R',
            b'M',
            b'8',
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            b'D',
            b'S',
            b'D',
            b' ',
            ..,
        ] => ContainerKind::Dsd,
        [b't', b'B', b'a', b'K', ..] => ContainerKind::Unsupported(Codec::Tak),
        [b'T', b'T', b'A', b'1', ..] => ContainerKind::Unsupported(Codec::Tta),
        head if head.starts_with(&ASF) => ContainerKind::Unsupported(Codec::Wma),
        _ => ContainerKind::Other,
    }
}
