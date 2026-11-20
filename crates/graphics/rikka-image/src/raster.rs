use std::borrow::Cow;
use std::sync::Arc;

use rikka_types::{Color, Size};

use crate::color_type::ColorType;
use crate::convert::convert;
use crate::error::{Error, Result};
use crate::pixels::{self, Pixels, Sample};
use crate::region::Region;

#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    width: u32,
    height: u32,
    color: ColorType,
    pub(crate) pixels: Pixels,
    icc: Option<Arc<[u8]>>,
}

impl Image {
    pub fn new(width: u32, height: u32, color: ColorType) -> Result<Image> {
        let bytes = checked_byte_len(width, height, color)?;
        let samples = bytes / usize::from(color.bits_per_channel() / 8);
        Ok(Image::from_pixels(
            width,
            height,
            color,
            Pixels::zeroed(color, samples),
        ))
    }

    pub fn filled(width: u32, height: u32, color: ColorType, fill: Color) -> Result<Image> {
        checked_byte_len(width, height, color)?;
        let pixel_count = width as usize * height as usize;
        Ok(Image::from_pixels(
            width,
            height,
            color,
            pixels::fill(color, fill, pixel_count),
        ))
    }

    pub fn from_rgba8(width: u32, height: u32, pixels: Vec<u8>) -> Result<Image> {
        Image::from_samples(width, height, ColorType::Rgba8, pixels)
    }

    pub fn from_rgb8(width: u32, height: u32, pixels: Vec<u8>) -> Result<Image> {
        Image::from_samples(width, height, ColorType::Rgb8, pixels)
    }

    pub fn from_rgba16(width: u32, height: u32, pixels: Vec<u16>) -> Result<Image> {
        Image::from_samples(width, height, ColorType::Rgba16, pixels)
    }

    pub fn from_rgba32f(width: u32, height: u32, pixels: Vec<f32>) -> Result<Image> {
        Image::from_samples(width, height, ColorType::Rgba32F, pixels)
    }

    pub(crate) fn from_samples<S: Sample>(
        width: u32,
        height: u32,
        color: ColorType,
        samples: Vec<S>,
    ) -> Result<Image> {
        let expected = checked_byte_len(width, height, color)?;
        let len = std::mem::size_of_val(samples.as_slice());
        if len != expected || usize::from(color.bits_per_channel() / 8) != size_of::<S>() {
            return Err(Error::PixelDataSize {
                len,
                expected,
                width,
                height,
                color,
            });
        }
        Ok(Image::from_pixels(width, height, color, S::wrap(samples)))
    }

    pub(crate) fn from_pixels(width: u32, height: u32, color: ColorType, pixels: Pixels) -> Image {
        debug_assert_eq!(
            pixels.len(),
            width as usize * height as usize * usize::from(color.channel_count())
        );
        Image {
            width,
            height,
            color,
            pixels,
            icc: None,
        }
    }

    pub(crate) fn derived(&self, width: u32, height: u32, pixels: Pixels) -> Image {
        Image {
            icc: self.icc.clone(),
            ..Image::from_pixels(width, height, self.color, pixels)
        }
    }

    pub(crate) fn replace_pixels(&mut self, width: u32, height: u32, pixels: Pixels) {
        *self = self.derived(width, height, pixels);
    }

    #[must_use]
    pub fn icc_profile(&self) -> Option<&[u8]> {
        self.icc.as_deref()
    }

    #[must_use]
    pub fn with_icc_profile(mut self, profile: Option<Arc<[u8]>>) -> Image {
        self.icc = profile.filter(|bytes| !bytes.is_empty());
        self
    }

    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub const fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    #[must_use]
    pub fn size(&self) -> Size {
        Size::new(self.width as f32, self.height as f32)
    }

    #[must_use]
    pub fn region(&self) -> Region {
        Region::from_size(self.width, self.height)
    }

    #[must_use]
    pub const fn color(&self) -> ColorType {
        self.color
    }

    #[must_use]
    pub fn has_alpha(&self) -> bool {
        self.color.has_alpha()
    }

    #[must_use]
    pub fn is_grayscale(&self) -> bool {
        !self.color.has_color()
    }

    #[must_use]
    pub fn bits_per_channel(&self) -> u8 {
        self.color.bits_per_channel()
    }

    #[must_use]
    pub fn pixel_count(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.pixels.as_bytes()
    }

    #[must_use]
    pub fn convert(&self, color: ColorType) -> Image {
        if self.color == color {
            return self.clone();
        }
        Image {
            icc: self.icc.clone(),
            ..Image::from_pixels(
                self.width,
                self.height,
                color,
                convert(&self.pixels, self.color, color),
            )
        }
    }

    pub(crate) fn in_color(&self, color: ColorType) -> Cow<'_, Image> {
        match self.color == color {
            true => Cow::Borrowed(self),
            false => Cow::Owned(self.convert(color)),
        }
    }

    #[must_use]
    pub fn rgba8(&self) -> Cow<'_, [u8]> {
        match (&self.pixels, self.color) {
            (Pixels::U8(samples), ColorType::Rgba8) => Cow::Borrowed(samples),
            _ => Cow::Owned(self.convert(ColorType::Rgba8).into_rgba8()),
        }
    }

    #[must_use]
    pub fn into_rgba8(self) -> Vec<u8> {
        match (self.pixels, self.color) {
            (Pixels::U8(samples), ColorType::Rgba8) => samples,
            (pixels, color) => match convert(&pixels, color, ColorType::Rgba8) {
                Pixels::U8(samples) => samples,
                _ => unreachable!("an 8-bit layout was requested"),
            },
        }
    }

    #[must_use]
    pub fn rgba32f(&self) -> Cow<'_, [f32]> {
        match (&self.pixels, self.color) {
            (Pixels::F32(samples), ColorType::Rgba32F) => Cow::Borrowed(samples),
            _ => Cow::Owned(self.convert(ColorType::Rgba32F).into_rgba32f()),
        }
    }

    #[must_use]
    pub fn into_rgba32f(self) -> Vec<f32> {
        match (self.pixels, self.color) {
            (Pixels::F32(samples), ColorType::Rgba32F) => samples,
            (pixels, color) => match convert(&pixels, color, ColorType::Rgba32F) {
                Pixels::F32(samples) => samples,
                _ => unreachable!("a float layout was requested"),
            },
        }
    }

    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> Option<Color> {
        (x < self.width && y < self.height).then(|| {
            let index = y as usize * self.width as usize + x as usize;
            pixels::color_at(&self.pixels, self.color, index)
        })
    }
}

pub(crate) fn checked_byte_len(width: u32, height: u32, color: ColorType) -> Result<usize> {
    let bytes = u64::from(width) * u64::from(height) * u64::from(color.bytes_per_pixel());
    if width == 0 || height == 0 || bytes > isize::MAX as u64 {
        return Err(Error::InvalidSize { width, height });
    }
    Ok(bytes as usize)
}
