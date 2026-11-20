use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Layout {
    L8,
    La8,
    Rgb8,
    Rgba8,
    L16,
    La16,
    Rgb16,
    Rgba16,
    Rgb32F,
    Rgba32F,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Depth {
    Eight,
    Sixteen,
    Float,
}

impl Layout {
    pub(crate) const fn new(gray: bool, alpha: bool, depth: Depth) -> Layout {
        match (depth, gray, alpha) {
            (Depth::Eight, true, false) => Layout::L8,
            (Depth::Eight, true, true) => Layout::La8,
            (Depth::Eight, false, false) => Layout::Rgb8,
            (Depth::Eight, false, true) => Layout::Rgba8,
            (Depth::Sixteen, true, false) => Layout::L16,
            (Depth::Sixteen, true, true) => Layout::La16,
            (Depth::Sixteen, false, false) => Layout::Rgb16,
            (Depth::Sixteen, false, true) => Layout::Rgba16,
            (Depth::Float, _, false) => Layout::Rgb32F,
            (Depth::Float, _, true) => Layout::Rgba32F,
        }
    }

    #[must_use]
    pub const fn channels(self) -> usize {
        match self {
            Layout::L8 | Layout::L16 => 1,
            Layout::La8 | Layout::La16 => 2,
            Layout::Rgb8 | Layout::Rgb16 | Layout::Rgb32F => 3,
            Layout::Rgba8 | Layout::Rgba16 | Layout::Rgba32F => 4,
        }
    }

    #[must_use]
    pub const fn has_alpha(self) -> bool {
        matches!(
            self,
            Layout::La8 | Layout::Rgba8 | Layout::La16 | Layout::Rgba16 | Layout::Rgba32F
        )
    }

    #[must_use]
    pub const fn is_gray(self) -> bool {
        matches!(self, Layout::L8 | Layout::La8 | Layout::L16 | Layout::La16)
    }

    pub(crate) const fn depth(self) -> Depth {
        match self {
            Layout::L8 | Layout::La8 | Layout::Rgb8 | Layout::Rgba8 => Depth::Eight,
            Layout::L16 | Layout::La16 | Layout::Rgb16 | Layout::Rgba16 => Depth::Sixteen,
            Layout::Rgb32F | Layout::Rgba32F => Depth::Float,
        }
    }

    #[must_use]
    pub const fn bytes_per_sample(self) -> usize {
        match self.depth() {
            Depth::Eight => 1,
            Depth::Sixteen => 2,
            Depth::Float => 4,
        }
    }

    #[must_use]
    pub const fn bytes_per_pixel(self) -> usize {
        self.channels() * self.bytes_per_sample()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Samples {
    U8(Vec<u8>),
    U16(Vec<u16>),
    F32(Vec<f32>),
}

impl Samples {
    pub(crate) fn zeroed(layout: Layout, samples: usize) -> Samples {
        match layout.depth() {
            Depth::Eight => Samples::U8(vec![0; samples]),
            Depth::Sixteen => Samples::U16(vec![0; samples]),
            Depth::Float => Samples::F32(vec![0.0; samples]),
        }
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Samples::U8(samples) => samples,
            Samples::U16(samples) => bytemuck::cast_slice(samples),
            Samples::F32(samples) => bytemuck::cast_slice(samples),
        }
    }

    pub(crate) fn as_bytes_mut(&mut self) -> &mut [u8] {
        match self {
            Samples::U8(samples) => samples,
            Samples::U16(samples) => bytemuck::cast_slice_mut(samples),
            Samples::F32(samples) => bytemuck::cast_slice_mut(samples),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub layout: Layout,
    pub samples: Samples,
}

impl Picture {
    pub(crate) fn zeroed(width: u32, height: u32, layout: Layout) -> Picture {
        let samples = width as usize * height as usize * layout.channels();
        Picture {
            width,
            height,
            layout,
            samples: Samples::zeroed(layout, samples),
        }
    }

    pub(crate) fn stride(&self) -> usize {
        self.width as usize * self.layout.bytes_per_pixel()
    }

    pub(crate) fn unpremultiply(&mut self) {
        let channels = self.layout.channels();
        if !self.layout.has_alpha() {
            return;
        }
        match &mut self.samples {
            Samples::U8(samples) => {
                for pixel in samples.chunks_exact_mut(channels) {
                    let (color, alpha) = pixel.split_at_mut(channels - 1);
                    let alpha = u32::from(alpha[0]);
                    for value in color.iter_mut().filter(|_| alpha > 0) {
                        *value = ((u32::from(*value) * 255 + alpha / 2) / alpha).min(255) as u8;
                    }
                }
            }
            Samples::U16(samples) => {
                for pixel in samples.chunks_exact_mut(channels) {
                    let (color, alpha) = pixel.split_at_mut(channels - 1);
                    let alpha = u64::from(alpha[0]);
                    for value in color.iter_mut().filter(|_| alpha > 0) {
                        *value =
                            ((u64::from(*value) * 65535 + alpha / 2) / alpha).min(65535) as u16;
                    }
                }
            }
            Samples::F32(samples) => {
                for pixel in samples.chunks_exact_mut(channels) {
                    let (color, alpha) = pixel.split_at_mut(channels - 1);
                    let alpha = alpha[0];
                    for value in color.iter_mut().filter(|_| alpha > 0.0) {
                        *value /= alpha;
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Pixels<'p> {
    pub width: u32,
    pub height: u32,
    pub layout: Layout,
    pub bytes: &'p [u8],
}

impl Pixels<'_> {
    pub(crate) fn stride(&self) -> usize {
        self.width as usize * self.layout.bytes_per_pixel()
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if self.width == 0 || self.height == 0 || self.width > i32::MAX as u32 {
            return Err(Error::InvalidSize {
                width: self.width,
                height: self.height,
            });
        }
        let expected = self.stride() * self.height as usize;
        if self.bytes.len() != expected {
            return Err(Error::PixelDataSize {
                len: self.bytes.len(),
                expected,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_width: u32,
    pub max_height: u32,
    pub max_bytes: u64,
}

impl Limits {
    pub const UNLIMITED: Limits = Limits {
        max_width: u32::MAX,
        max_height: u32::MAX,
        max_bytes: u64::MAX,
    };

    pub(crate) fn admit(&self, width: u32, height: u32, layout: Layout) -> Result<u64> {
        if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
            return Err(Error::InvalidSize { width, height });
        }
        let over = Error::LimitsExceeded { width, height };
        if width > self.max_width || height > self.max_height {
            return Err(over);
        }
        let bytes = u64::from(width) * u64::from(height) * layout.bytes_per_pixel() as u64;
        if bytes > self.max_bytes || bytes > isize::MAX as u64 {
            return Err(over);
        }
        Ok(bytes)
    }

    pub(crate) fn admit_total(&self, width: u32, height: u32, bytes: u64) -> Result<()> {
        match bytes > self.max_bytes {
            true => Err(Error::LimitsExceeded { width, height }),
            false => Ok(()),
        }
    }
}
