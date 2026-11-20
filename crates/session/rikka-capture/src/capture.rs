use std::time::Instant;

use crate::buffer::CaptureBuffer;
use crate::error::{Error, Result};
use crate::format::PixelFormat;
use crate::frames::{CaptureEvent, FrameFailure, Frames};
use crate::screen::Screen;
use crate::source::{Region, Source};

const CONSTRAINT_RETRIES: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Frame {
    Drawn,
    ConstraintsChanged,
    Nothing,
}

pub struct Capturer {
    screen: Screen,
    frames: Frames,
    buffer: Option<CaptureBuffer>,
    generation: u64,
    region: Option<Region>,
}

impl Capturer {
    pub fn open(mut screen: Screen, source: &Source, cursor: bool) -> Result<Self> {
        let mut frames = screen.frames(source, cursor)?;
        screen.negotiate(&mut frames)?;

        Ok(Self {
            screen,
            frames,
            buffer: None,
            generation: 0,
            region: None,
        })
    }

    pub fn crop(&mut self, region: Option<Region>) -> Result<()> {
        let (width, height) = self.source_size();
        if let Some(region) = region
            && !region.fits(width, height)
        {
            return Err(Error::Region {
                x: region.x,
                y: region.y,
                width: region.width,
                height: region.height,
                source_width: width,
                source_height: height,
            });
        }
        self.region = region;
        Ok(())
    }

    pub fn source_size(&self) -> (u32, u32) {
        let constraints = self.frames.constraints();
        (constraints.width, constraints.height)
    }

    pub fn frame_size(&self) -> (u32, u32) {
        match self.region {
            Some(region) => (region.width, region.height),
            None => self.source_size(),
        }
    }

    pub fn capture_into(&mut self, out: &mut Vec<u8>) -> Result<(u32, u32)> {
        match self.capture_before(out, None)? {
            Some(size) => Ok(size),
            None => Err(Error::Failed("capture completed without a frame")),
        }
    }

    pub fn capture_before(
        &mut self,
        out: &mut Vec<u8>,
        deadline: Option<Instant>,
    ) -> Result<Option<(u32, u32)>> {
        let Some(frame) = self.take_before(deadline)? else {
            return Ok(None);
        };
        let size = (frame.width, frame.height);
        copy_rgba(&frame, out);
        Ok(Some(size))
    }

    pub fn take_before(&mut self, deadline: Option<Instant>) -> Result<Option<CapturedFrame<'_>>> {
        let mut drawn = false;
        for _ in 0..CONSTRAINT_RETRIES {
            self.reallocate()?;
            match self.one_frame(deadline)? {
                Frame::Drawn => {
                    drawn = true;
                    break;
                }
                Frame::Nothing => return Ok(None),
                Frame::ConstraintsChanged => continue,
            }
        }
        match drawn {
            true => Ok(self.take_frame()),
            false => Err(Error::Failed(
                "capture session kept renegotiating buffer constraints",
            )),
        }
    }

    fn take_frame(&self) -> Option<CapturedFrame<'_>> {
        let buffer = self.buffer.as_ref()?;
        let source = buffer.pixels()?;
        let stride = buffer.stride() as usize;
        let request = self
            .region
            .unwrap_or(Region::new(0, 0, buffer.width(), buffer.height()));
        let (width, height) = (request.width as usize, request.height as usize);
        if width == 0 || height == 0 {
            return None;
        }
        let start = request.y as usize * stride + request.x as usize * 4;
        let len = (height - 1) * stride + width * 4;
        Some(CapturedFrame {
            bytes: source.get(start..start + len)?,
            stride: stride as u32,
            format: buffer.format(),
            width: request.width,
            height: request.height,
        })
    }

    fn one_frame(&mut self, deadline: Option<Instant>) -> Result<Frame> {
        let buffer = self
            .buffer
            .as_ref()
            .ok_or(Error::Failed("no capture buffer allocated"))?;
        self.frames.capture(buffer)?;

        loop {
            if !self.screen.dispatch(deadline)? {
                self.frames.cancel();
                return Ok(Frame::Nothing);
            }
            let mut drawn = false;
            let mut restated = false;
            let mut failed = None;
            for event in self.frames.events() {
                match event {
                    CaptureEvent::Ready(_) => drawn = true,
                    CaptureEvent::Failed(FrameFailure::BufferConstraints)
                    | CaptureEvent::ConstraintsChanged => {
                        restated = true;
                    }
                    CaptureEvent::Failed(FrameFailure::Stopped) | CaptureEvent::Stopped => {
                        failed = Some(Error::Stopped);
                    }
                    CaptureEvent::Failed(FrameFailure::Unknown) => {
                        failed = Some(Error::Failed("compositor failed to render frame"));
                    }
                }
            }
            if let Some(failure) = failed {
                self.frames.cancel();
                return Err(failure);
            }
            if drawn {
                return Ok(Frame::Drawn);
            }
            if restated {
                self.frames.cancel();
                return Ok(Frame::ConstraintsChanged);
            }
        }
    }

    fn reallocate(&mut self) -> Result<()> {
        let constraints = self.frames.constraints();
        if self.buffer.is_some() && self.generation == constraints.generation {
            return Ok(());
        }
        if constraints.is_empty() {
            return Err(Error::Failed("source has zero size"));
        }
        let format = constraints.best_shm().ok_or(Error::NoFormat)?;

        self.buffer = None;
        self.buffer = Some(self.screen.shm_buffer(
            format,
            constraints.width,
            constraints.height,
        )?);
        self.generation = constraints.generation;
        if let Some(region) = self.region
            && !region.fits(constraints.width, constraints.height)
        {
            self.region = None;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct CapturedFrame<'a> {
    pub bytes: &'a [u8],
    pub stride: u32,
    pub format: PixelFormat,
    pub width: u32,
    pub height: u32,
}

fn copy_rgba(frame: &CapturedFrame<'_>, out: &mut Vec<u8>) {
    let (width, height) = (frame.width as usize, frame.height as usize);
    let stride = frame.stride as usize;
    let swap_red_blue = frame.format.is_bgr();

    out.clear();
    out.reserve(width * height * 4);
    for row in 0..height {
        let start = row * stride;
        let length = out.len();
        out.extend_from_slice(&frame.bytes[start..start + width * 4]);
        for pixel in out[length..].as_chunks_mut::<4>().0 {
            if swap_red_blue {
                pixel.swap(0, 2);
            }
            pixel[3] = 0xff;
        }
    }
}

impl std::fmt::Debug for Capturer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Capturer")
            .field("size", &self.frame_size())
            .finish_non_exhaustive()
    }
}
