use std::path::Path;

use rikka_image::{ColorType, Filter, Format, Image, Png};

use crate::error::{Error, Result};
use crate::framing::Framing;

pub const AVATAR_SIDE: u32 = 512;

pub const TRANSPARENT_SIDE: u32 = 480;

const OPAQUE: u8 = u8::MAX;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preview {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug)]
pub struct Photo {
    image: Image,
}

impl Photo {
    pub fn open(path: &Path) -> Result<Photo> {
        Photo::from_image(Image::open(path)?)
    }

    pub fn from_image(image: Image) -> Result<Photo> {
        match image.width() == 0 || image.height() == 0 {
            true => Err(Error::Empty),
            false => Ok(Photo { image }),
        }
    }

    #[must_use]
    pub fn mime_types() -> Vec<&'static str> {
        let mut types: Vec<&'static str> = Format::ALL
            .into_iter()
            .flat_map(Format::mime_types)
            .copied()
            .collect();
        types.sort_unstable();
        types.dedup();
        types
    }

    #[must_use]
    pub fn framing(&self) -> Framing {
        Framing::new(self.image.width(), self.image.height())
    }

    pub fn preview(&self, longest: u32) -> Result<Preview> {
        let shown = match self.image.width().max(self.image.height()) > longest {
            true => self
                .image
                .resize_to_fit(longest, longest, Filter::Lanczos3)?,
            false => self.image.convert(ColorType::Rgba8),
        };
        Ok(Preview {
            width: shown.width(),
            height: shown.height(),
            rgba: shown.into_rgba8(),
        })
    }

    pub fn avatar(&self, framing: &Framing) -> Result<Vec<u8>> {
        let cropped = self.image.crop(framing.region())?;
        let transparent = cropped.has_alpha()
            && cropped
                .rgba8()
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] != OPAQUE);
        let (side, color) = match transparent {
            true => (TRANSPARENT_SIDE, ColorType::Rgba8),
            false => (AVATAR_SIDE, ColorType::Rgb8),
        };
        let avatar = cropped
            .resize_exact(side, side, Filter::Lanczos3)?
            .convert(color);
        Ok(avatar.encode(Png)?)
    }
}
