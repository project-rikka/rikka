use std::sync::Arc;

use crate::error::{Error, Result};

const LARGEST_SIDE: u32 = 4096;

const CHANNELS: u32 = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    width: u32,
    height: u32,
    rgba: Arc<[u8]>,
}

impl Picture {
    pub fn rgba(width: u32, height: u32, rgba: impl Into<Arc<[u8]>>) -> Result<Picture> {
        let rgba = rgba.into();
        let expected = u64::from(width) * u64::from(height) * u64::from(CHANNELS);
        let sides = 1..=LARGEST_SIDE;
        if !sides.contains(&width) || !sides.contains(&height) || rgba.len() as u64 != expected {
            return Err(Error::Picture {
                width,
                height,
                bytes: rgba.len(),
            });
        }
        Ok(Picture {
            width,
            height,
            rgba,
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
    pub fn pixels(&self) -> &[u8] {
        &self.rgba
    }

    pub(crate) fn stride(&self) -> u32 {
        self.width * CHANNELS
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Icon {
    Named(String),
    Picture(Picture),
}

impl Icon {
    pub fn named(name: impl Into<String>) -> Icon {
        Icon::Named(name.into())
    }
}

impl From<Picture> for Icon {
    fn from(picture: Picture) -> Icon {
        Icon::Picture(picture)
    }
}
