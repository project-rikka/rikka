mod cover;
mod detail;
mod search;

use std::path::{Path, PathBuf};

use rikka_image::{Image, Orientation};
use rikka_types::PixelSize;

use crate::error::Result;
use crate::poster::search::Search;
use crate::shaping::{Shaping, orientation};
use rikka_ffmpeg::Frame;
use rikka_video_core::demux::Demuxer;
use rikka_video_core::init;
use rikka_video_core::sw::SwConverter;

#[derive(Clone, Debug)]
#[must_use]
pub struct Poster {
    path: PathBuf,
    bounds: Option<(u32, u32)>,
}

pub fn poster(path: impl AsRef<Path>) -> Poster {
    Poster {
        path: path.as_ref().to_path_buf(),
        bounds: None,
    }
}

impl Poster {
    pub fn with_max_size(mut self, size: PixelSize) -> Poster {
        self.bounds = Some((size.width.max(1), size.height.max(1)));
        self
    }

    pub fn decode(self) -> Result<Image> {
        init::ensure()?;
        let mut demuxer = Demuxer::open(&self.path)?;
        let mut converter = SwConverter::new();

        if let Some(cover) = cover::largest(&demuxer) {
            let shaping = Shaping {
                stored_aspect: cover.sample_aspect_ratio,
                orientation: Orientation::Upright,
                bounds: self.bounds,
            };
            let mut frame = Frame::new()?;
            match cover::decode(cover, &mut frame)
                .and_then(|()| shaping.picture(&mut converter, &frame))
            {
                Ok(picture) => return Ok(picture),
                Err(error) => tracing::debug!(
                    "failed to decode cover art of {}, using a frame: {error}",
                    self.path.display()
                ),
            }
        }

        let stream = demuxer.stream();
        let shaping = Shaping {
            stored_aspect: stream.sample_aspect_ratio,
            orientation: orientation::from_stream(stream),
            bounds: self.bounds,
        };
        let chosen = Search::new(&mut demuxer)?.run()?;
        shaping.picture(&mut converter, &chosen)
    }
}
