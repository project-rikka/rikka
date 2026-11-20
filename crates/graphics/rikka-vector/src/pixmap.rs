use crate::error::{Error, Result};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pixmap {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl Pixmap {
    pub fn new(width: u32, height: u32) -> Result<Pixmap> {
        let len = byte_len(width, height).ok_or(Error::InvalidSize { width, height })?;
        Ok(Pixmap {
            width,
            height,
            data: vec![0; len],
        })
    }

    pub fn from_premultiplied(width: u32, height: u32, data: Vec<u8>) -> Result<Pixmap> {
        match byte_len(width, height) == Some(data.len()) {
            true => Ok(Pixmap {
                width,
                height,
                data,
            }),
            false => Err(Error::InvalidSize { width, height }),
        }
    }

    pub fn from_straight(width: u32, height: u32, mut data: Vec<u8>) -> Result<Pixmap> {
        for pixel in data.as_chunks_mut::<4>().0 {
            let alpha = u32::from(pixel[3]);
            if alpha < 255 {
                for channel in &mut pixel[..3] {
                    *channel = ((u32::from(*channel) * alpha + 127) / 255) as u8;
                }
            }
        }
        Pixmap::from_premultiplied(width, height, data)
    }

    #[inline]
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[inline]
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[inline]
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    #[inline]
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    #[must_use]
    pub fn into_premultiplied(self) -> Vec<u8> {
        self.data
    }

    #[must_use]
    pub fn into_straight(mut self) -> Vec<u8> {
        for pixel in self.data.as_chunks_mut::<4>().0 {
            let alpha = u32::from(pixel[3]);
            if alpha > 0 && alpha < 255 {
                for channel in &mut pixel[..3] {
                    *channel = ((u32::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
                }
            }
        }
        self.data
    }
}

fn byte_len(width: u32, height: u32) -> Option<usize> {
    let bytes = u64::from(width) * u64::from(height) * 4;
    (width > 0 && height > 0 && bytes <= isize::MAX as u64).then_some(bytes as usize)
}
