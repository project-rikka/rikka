use std::hash::{Hash as _, Hasher as _};
use std::path::Path;

use rikka_capture::{Screen, ShotOptions, Source};
use rikka_image::{Filter, Image, Png};

use crate::error::{Error, Result, capture};
use crate::region::Region;

pub(crate) const CURSOR: bool = true;

#[derive(Clone)]
pub struct Shot {
    width: u32,
    height: u32,
    png: Vec<u8>,
}

pub(crate) enum Screenshot<'a> {
    Socket(&'a Path),
    Output(&'a Source),
}

impl Screenshot<'_> {
    pub(crate) fn shot(&self, size: (u32, u32)) -> Result<Shot> {
        let captured = self.capture()?;
        let sized = match captured.dimensions() == size {
            true => captured,
            false => captured
                .resize_to_fit(size.0, size.1, Filter::Lanczos3)
                .map_err(capture)?,
        };
        Shot::encode(&sized)
    }

    pub(crate) fn zoom(&self, size: (u32, u32), region: Region) -> Result<Shot> {
        if !region.fits(size) {
            return Err(Error::RegionOutside {
                region,
                width: size.0,
                height: size.1,
            });
        }
        let captured = self.capture()?;
        let cropped = captured
            .crop(region.scaled(size, captured.dimensions()))
            .map_err(capture)?;
        let zoomed = cropped
            .resize_to_fit(size.0, size.1, Filter::Lanczos3)
            .map_err(capture)?;
        Shot::encode(&zoomed)
    }

    fn capture(&self) -> Result<Image> {
        let options = ShotOptions::default().cursor(CURSOR);
        let raw = match self {
            Screenshot::Socket(socket) => {
                let screen = Screen::connect_to(socket).map_err(capture)?;
                rikka_capture::Shot::take_on(screen, &Source::FirstOutput, options)
            }
            Screenshot::Output(source) => rikka_capture::Shot::take(source, options),
        }
        .map_err(capture)?;
        Image::from_rgba8(raw.width, raw.height, raw.pixels).map_err(capture)
    }
}

impl Shot {
    fn encode(image: &Image) -> Result<Shot> {
        Ok(Shot {
            width: image.width(),
            height: image.height(),
            png: image.encode(Png).map_err(capture)?,
        })
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub fn fingerprint(&self) -> u64 {
        let mut hasher = std::hash::DefaultHasher::new();
        self.png.hash(&mut hasher);
        hasher.finish()
    }

    #[must_use]
    pub fn into_png(self) -> Vec<u8> {
        self.png
    }
}

impl std::fmt::Debug for Shot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Shot")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("bytes", &self.png.len())
            .finish()
    }
}
