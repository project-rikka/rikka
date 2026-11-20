use crate::error::{Error, Result};
use crate::texture::ring::RingSpan;
use crate::texture::{TextureId, Uploads};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TexelRect {
    pub offset: [u32; 2],
    pub extent: [u32; 2],
}

impl TexelRect {
    #[must_use]
    pub const fn new(offset: [u32; 2], extent: [u32; 2]) -> Self {
        Self { offset, extent }
    }

    pub(crate) fn fits(&self, whole: [u32; 2]) -> bool {
        let fits = |at: u32, size: u32, whole: u32| {
            size > 0 && at.checked_add(size).is_some_and(|end| end <= whole)
        };
        fits(self.offset[0], self.extent[0], whole[0])
            && fits(self.offset[1], self.extent[1], whole[1])
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Rows<'a> {
    pixels: &'a [u8],
    stride: usize,
}

impl<'a> Rows<'a> {
    #[must_use]
    pub fn new(pixels: &'a [u8], stride: usize) -> Self {
        Self { pixels, stride }
    }

    pub(crate) fn copy_into(
        &self,
        rect: TexelRect,
        texel_bytes: usize,
        into: &mut [u8],
    ) -> Result<()> {
        let [left, top] = rect.offset.map(|at| at as usize);
        let [width, height] = rect.extent.map(|size| size as usize);
        let row = width * texel_bytes;
        let first = left * texel_bytes;
        let needed = (top + height - 1) * self.stride + first + row;
        if self.stride < first + row || self.pixels.len() < needed {
            return Err(Error::Invalid(format!(
                "{} bytes with stride {} are too small for {width}x{height} texels at {left},{top}",
                self.pixels.len(),
                self.stride
            )));
        }
        let start = top * self.stride + first;
        if row == self.stride {
            into.copy_from_slice(&self.pixels[start..start + row * height]);
            return Ok(());
        }
        for (line, to) in into.chunks_exact_mut(row).enumerate() {
            let from = start + line * self.stride;
            to.copy_from_slice(&self.pixels[from..from + row]);
        }
        Ok(())
    }
}

#[must_use = "staged rows reach their texture only once sent"]
pub struct Staged {
    pub(crate) uploads: Uploads,
    pub(crate) id: TextureId,
    pub(crate) parts: Vec<(TexelRect, RingSpan)>,
}

impl Staged {
    pub fn send(self) -> Result<()> {
        let Staged { uploads, id, parts } = self;
        uploads.send_staged(id, parts)
    }
}

impl std::fmt::Debug for Staged {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Staged")
            .field("id", &self.id)
            .field("rects", &self.parts.len())
            .finish()
    }
}
