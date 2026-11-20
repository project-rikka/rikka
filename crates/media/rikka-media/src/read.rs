use std::fs::File;
use std::path::Path;
use std::time::Duration;

use rikka_image::Image;
use rikka_media_format::{Detail, Extent, Format, Kind, identify, kind};
use rikka_video::PixelSize;

use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Info {
    pub format: Format,
    pub extent: Option<Extent>,
    pub duration: Option<Duration>,
    pub bytes: u64,
}

impl Info {
    #[must_use]
    pub fn kind(self) -> Kind {
        self.format.kind()
    }
}

pub fn still(path: impl AsRef<Path>) -> Result<Image> {
    picture(path.as_ref(), None)
}

pub fn still_with_max_size(path: impl AsRef<Path>, max_size: PixelSize) -> Result<Image> {
    picture(path.as_ref(), Some((max_size.width, max_size.height)))
}

fn picture(path: &Path, bounds: Option<(u32, u32)>) -> Result<Image> {
    match kind(path) {
        Some(Kind::Picture) => drawn(path, bounds),
        Some(Kind::Movie) => poster(path, bounds),
        Some(Kind::Sound) => Err(Error::NoPicture(path.to_path_buf())),
        None => drawn(path, bounds).or_else(|_| poster(path, bounds)),
    }
}

fn drawn(path: &Path, bounds: Option<(u32, u32)>) -> Result<Image> {
    let image = rikka_image::read(path).decode()?;
    match bounds {
        Some((width, height)) => Ok(image.thumbnail(width, height)?),
        None => Ok(image),
    }
}

fn poster(path: &Path, bounds: Option<(u32, u32)>) -> Result<Image> {
    let poster = rikka_video::poster(path);
    let poster = match bounds {
        Some((width, height)) => poster.with_max_size(PixelSize::new(width, height)),
        None => poster,
    };
    Ok(poster.decode()?)
}

pub fn info(path: impl AsRef<Path>) -> Result<Info> {
    let path = path.as_ref();
    let file = File::open(path)?;
    let bytes = file.metadata()?.len();
    let Some(format) = identify(path) else {
        return Err(Error::Unsupported(path.to_path_buf()));
    };

    let (extent, duration) = match format.detail() {
        Detail::Picture(_) => {
            let picture = rikka_image::read(path).probe()?;
            (Extent::new(picture.width, picture.height), None)
        }
        Detail::Sound(hint) => {
            let sound = rikka_audio_decode::read(path);
            let sound = match hint {
                Some(hint) => sound.format(hint),
                None => sound,
            };
            (None, sound.probe()?.duration)
        }
        Detail::Movie => {
            let movie = rikka_video::probe(path)?;
            (Extent::new(movie.width, movie.height), movie.duration)
        }
    };

    Ok(Info {
        format,
        extent,
        duration,
        bytes,
    })
}
