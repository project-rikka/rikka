use crate::error::{Error, Result};

const MAX_SIDE: u32 = i32::MAX as u32 / 4;

#[derive(Clone, Copy)]
pub struct Picture<'a> {
    width: u32,
    height: u32,
    pixels: &'a [u8],
}

impl<'a> Picture<'a> {
    pub fn rgba(width: u32, height: u32, pixels: &'a [u8]) -> Result<Picture<'a>> {
        if width == 0 || height == 0 {
            return Err(Error::Empty);
        }
        let expected = u64::from(width) * u64::from(height) * 4;
        if width > MAX_SIDE || height > MAX_SIDE || expected > u64::from(MAX_SIDE) {
            return Err(Error::TooLarge { width, height });
        }
        if pixels.len() as u64 != expected {
            return Err(Error::Mismatched {
                width,
                height,
                length: pixels.len(),
            });
        }
        Ok(Picture {
            width,
            height,
            pixels,
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

    pub(crate) fn luma(&self) -> Luma {
        let pixels = self
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .map(|&[red, green, blue, _]| {
                let weighted =
                    306 * u32::from(red) + 601 * u32::from(green) + 117 * u32::from(blue);
                ((weighted + 0x200) >> 10) as u8
            })
            .collect();
        Luma {
            width: self.width as usize,
            height: self.height as usize,
            pixels,
        }
    }
}

impl std::fmt::Debug for Picture<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Picture")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

pub(crate) struct Luma {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) pixels: Vec<u8>,
}

impl Luma {
    pub(crate) fn upscaled(self, factor: usize) -> Luma {
        if factor <= 1 {
            return self;
        }
        let width = self.width * factor;
        let mut pixels = Vec::with_capacity(width * self.height * factor);
        for row in self.pixels.chunks_exact(self.width) {
            let start = pixels.len();
            pixels.extend(
                row.iter()
                    .flat_map(|&shade| std::iter::repeat_n(shade, factor)),
            );
            for _ in 1..factor {
                pixels.extend_from_within(start..start + width);
            }
        }
        Luma {
            width,
            height: self.height * factor,
            pixels,
        }
    }

    pub(crate) fn padded(&self, margin: usize, shade: u8) -> Luma {
        let width = self.width + margin * 2;
        let height = self.height + margin * 2;
        let mut pixels = vec![shade; width * height];
        for (line, row) in self.pixels.chunks_exact(self.width).enumerate() {
            let start = (line + margin) * width + margin;
            pixels[start..start + self.width].copy_from_slice(row);
        }
        Luma {
            width,
            height,
            pixels,
        }
    }

    pub(crate) fn mean_corner_luma(&self) -> u8 {
        let last_row = (self.height - 1) * self.width;
        let corners = [0, self.width - 1, last_row, last_row + self.width - 1];
        let sum: u32 = corners.iter().map(|&at| u32::from(self.pixels[at])).sum();
        (sum / 4) as u8
    }
}
