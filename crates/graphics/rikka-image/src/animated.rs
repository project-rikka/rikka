use std::time::Duration;

use rikka_types::Size;

use crate::codecs::Limits;
use crate::color_type::ColorType;
use crate::error::{Error, Result};
use crate::frame::Frame;
use crate::loops::Loops;
use crate::raster::Image;

const MIN_DELAY: Duration = Duration::from_millis(20);

pub const DEFAULT_DELAY: Duration = Duration::from_millis(100);

#[derive(Clone, Debug)]
pub struct AnimatedImage {
    frames: Vec<Frame>,
    width: u32,
    height: u32,
    loops: Loops,
}

impl AnimatedImage {
    pub(crate) fn from_still(image: Image) -> AnimatedImage {
        let (width, height) = image.dimensions();
        AnimatedImage {
            frames: vec![Frame::new(image, Duration::ZERO)],
            width,
            height,
            loops: Loops::Finite(1),
        }
    }

    #[must_use]
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }

    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    #[must_use]
    pub fn is_animated(&self) -> bool {
        self.frames.len() > 1
    }

    #[must_use]
    pub fn still(&self) -> &Image {
        self.frames
            .first()
            .map(Frame::image)
            .unwrap_or_else(|| unreachable!("animation has at least one frame"))
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
    pub const fn loops(&self) -> Loops {
        self.loops
    }

    #[must_use]
    pub fn into_frames(self) -> Vec<Frame> {
        self.frames
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Dispose {
    #[default]
    Keep,
    Background,
    Previous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Blend {
    Source,
    Over,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Patch<'p> {
    pub(crate) left: u32,
    pub(crate) top: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: &'p [u8],
}

#[derive(Debug)]
pub(crate) struct Compositor {
    width: u32,
    height: u32,
    canvas: Vec<u8>,
    saved: Vec<u8>,
    frames: Vec<Frame>,
    allocated: u64,
    budget: Option<u64>,
}

impl Compositor {
    pub(crate) fn new(width: u32, height: u32, limits: &Limits) -> Result<Compositor> {
        let bytes = limits.admit(width, height, ColorType::Rgba8)?;
        Ok(Compositor {
            width,
            height,
            canvas: vec![0; bytes],
            saved: Vec::new(),
            frames: Vec::new(),
            allocated: 0,
            budget: limits.max_alloc,
        })
    }

    pub(crate) fn push(
        &mut self,
        patch: Patch<'_>,
        blend: Blend,
        dispose: Dispose,
        delay: Duration,
    ) -> Result<()> {
        let canvas_bytes = self.canvas.len() as u64;
        self.allocated = self.allocated.saturating_add(canvas_bytes);
        if let Some(budget) = self.budget
            && self.allocated > budget
        {
            return Err(Error::LimitsExceeded(format!(
                "animation frames exceed {budget} bytes"
            )));
        }
        let columns = patch.width.min(self.width.saturating_sub(patch.left)) as usize;
        let rows = match columns {
            0 => 0,
            _ => patch.height.min(self.height.saturating_sub(patch.top)) as usize,
        };
        let pitch = self.width as usize * 4;
        let span = |row: usize| {
            let start = (patch.top as usize + row) * pitch + patch.left as usize * 4;
            start..start + columns * 4
        };
        let dispose = match (dispose, self.frames.is_empty()) {
            (Dispose::Previous, true) => Dispose::Background,
            (dispose, _) => dispose,
        };
        if dispose == Dispose::Previous {
            self.saved.clear();
            for row in 0..rows {
                self.saved.extend_from_slice(&self.canvas[span(row)]);
            }
        }
        let source_pitch = patch.width as usize * 4;
        for row in 0..rows {
            let source = &patch.rgba[row * source_pitch..row * source_pitch + columns * 4];
            let target = &mut self.canvas[span(row)];
            match blend {
                Blend::Source => target.copy_from_slice(source),
                Blend::Over => over(target, source),
            }
        }
        let image = Image::from_rgba8(self.width, self.height, self.canvas.clone())?;
        let delay = if delay < MIN_DELAY {
            DEFAULT_DELAY
        } else {
            delay
        };
        self.frames.push(Frame::new(image, delay));
        match dispose {
            Dispose::Keep => {}
            Dispose::Background => {
                for row in 0..rows {
                    self.canvas[span(row)].fill(0);
                }
            }
            Dispose::Previous => {
                for (row, saved) in self.saved.chunks_exact((columns * 4).max(1)).enumerate() {
                    self.canvas[span(row)].copy_from_slice(saved);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn finish(self, loops: Loops) -> Result<AnimatedImage> {
        if self.frames.is_empty() {
            return Err(Error::Decoding("animation has no frames".to_owned()));
        }
        Ok(AnimatedImage {
            frames: self.frames,
            width: self.width,
            height: self.height,
            loops,
        })
    }
}

fn over(target: &mut [u8], source: &[u8]) {
    for (under, top) in target
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(source.as_chunks::<4>().0)
    {
        match top[3] {
            0 => {}
            255 => *under = *top,
            _ => {
                let top_alpha = f32::from(top[3]) / 255.0;
                let under_alpha = f32::from(under[3]) / 255.0;
                let alpha = top_alpha + under_alpha * (1.0 - top_alpha);
                for channel in 0..3 {
                    let mixed = f32::from(top[channel]) * top_alpha
                        + f32::from(under[channel]) * under_alpha * (1.0 - top_alpha);
                    under[channel] = (mixed / alpha).round() as u8;
                }
                under[3] = (alpha * 255.0).round() as u8;
            }
        }
    }
}
