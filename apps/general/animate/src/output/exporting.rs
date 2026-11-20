use animate_cast::Resolution;
use animate_export::{BITRATES, Encoding, Export, Rate};
use rikka_app::prelude::*;
use rikka_model::mmd::FRAMES_PER_SECOND;

use super::sizing::Sizing;

const LAST_FRAME: u32 = FRAMES_PER_SECOND as u32 * 60 * 60 * 4;

#[derive(Debug)]
pub(crate) struct Exporting {
    sizing: Sizing,
    encoding: Encoding,
    first: TextDocument,
    last: TextDocument,
    bitrate: TextDocument,
    sound: bool,
    asking: bool,
}

impl Exporting {
    pub(super) fn new(
        resolution: Resolution,
        encoding: Encoding,
        last_frame: u32,
        sound: bool,
        key: u64,
    ) -> Self {
        Self {
            sizing: Sizing::new(resolution, key),
            encoding,
            first: TextDocument::new("0"),
            last: TextDocument::new(last_frame.to_string()),
            bitrate: TextDocument::new(encoding.bitrate.to_string()),
            sound,
            asking: false,
        }
    }

    pub(crate) fn key(&self) -> u64 {
        self.sizing.key()
    }

    pub(crate) fn sizing(&self) -> &Sizing {
        &self.sizing
    }

    pub(super) fn sizing_mut(&mut self) -> &mut Sizing {
        &mut self.sizing
    }

    pub(crate) fn encoding(&self) -> &Encoding {
        &self.encoding
    }

    pub(crate) fn encoding_mut(&mut self) -> &mut Encoding {
        &mut self.encoding
    }

    pub(crate) fn first(&self) -> &TextDocument {
        &self.first
    }

    pub(crate) fn last(&self) -> &TextDocument {
        &self.last
    }

    pub(crate) fn bitrate(&self) -> &TextDocument {
        &self.bitrate
    }

    pub(crate) fn has_sound(&self) -> bool {
        self.sound
    }

    pub(crate) fn is_asking(&self) -> bool {
        self.asking
    }

    pub(crate) fn ask(&mut self, asking: bool) {
        self.asking = asking;
    }

    pub(crate) fn resolution(&self) -> Option<Resolution> {
        self.sizing.drafted().filter(|drafted| drafted.is_even())
    }

    pub(crate) fn range(&self) -> Option<(u32, u32)> {
        let first = frame(&self.first.text())?;
        let last = frame(&self.last.text())?;
        (first <= last).then_some((first, last))
    }

    pub(crate) fn typed_bitrate(&self) -> Option<u32> {
        self.bitrate
            .text()
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|bitrate| BITRATES.contains(bitrate))
    }

    pub(crate) fn export(&self) -> Option<Export> {
        let resolution = self.resolution()?;
        let (first, last) = self.range()?;
        let mut encoding = self.encoding;
        match encoding.rate {
            Rate::Bitrate => encoding.bitrate = self.typed_bitrate()?,
            Rate::Quality => {
                if let Some(bitrate) = self.typed_bitrate() {
                    encoding.bitrate = bitrate;
                }
            }
        }
        Some(Export {
            resolution,
            encoding,
            first,
            last,
        })
    }
}

pub(crate) fn frame(typed: &str) -> Option<u32> {
    typed
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|frame| *frame <= LAST_FRAME)
}
