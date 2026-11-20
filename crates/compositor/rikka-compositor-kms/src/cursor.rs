use crate::allocator::{Allocator, Cursor};
use crate::error::{Error, Result};
use crate::framebuffer::Framebuffer;

const BYTES_PER_PIXEL: usize = 4;

#[derive(Debug, Clone)]
pub(crate) struct CursorImage {
    pub(crate) framebuffer: Framebuffer,
    pub(crate) hotspot: (i32, i32),
}

pub(crate) struct CursorPool {
    plane: (u32, u32),
    buffers: Vec<(Cursor, Framebuffer)>,
    staging: Staging,
}

#[derive(Debug, Default)]
struct Staging {
    bytes: Vec<u8>,
    stride: usize,
    painted: (usize, usize),
}

impl CursorPool {
    pub(crate) fn new(plane: (u32, u32)) -> Self {
        Self {
            plane,
            buffers: Vec::new(),
            staging: Staging::default(),
        }
    }

    pub(crate) fn draw(
        &mut self,
        allocator: &Allocator,
        width: u32,
        height: u32,
        argb: &[u8],
        hotspot: (i32, i32),
    ) -> Result<CursorImage> {
        let (limit_width, limit_height) = self.plane;
        if width > limit_width || height > limit_height {
            return Err(Error::CursorTooLarge {
                width,
                height,
                limit_width,
                limit_height,
            });
        }
        let expected = width as usize * BYTES_PER_PIXEL * height as usize;
        if argb.len() != expected {
            return Err(Error::CursorPixels {
                got: argb.len(),
                expected,
            });
        }
        let index = match self
            .buffers
            .iter()
            .position(|(_, framebuffer)| !framebuffer.is_held_elsewhere())
        {
            Some(index) => index,
            None => {
                self.buffers
                    .push(allocator.cursor(limit_width, limit_height)?);
                self.buffers.len() - 1
            }
        };
        let (buffer, framebuffer) = &mut self.buffers[index];
        self.staging.paint(
            buffer.stride() as usize,
            limit_height as usize,
            width as usize,
            argb,
        );
        buffer.write(&self.staging.bytes)?;
        Ok(CursorImage {
            framebuffer: framebuffer.clone(),
            hotspot,
        })
    }
}

impl Staging {
    fn paint(&mut self, stride: usize, rows: usize, width: usize, argb: &[u8]) {
        if self.stride != stride || self.bytes.len() != stride * rows {
            self.bytes.clear();
            self.bytes.resize(stride * rows, 0);
            self.stride = stride;
            self.painted = (0, 0);
        }
        let (painted_rows, painted_row) = self.painted;
        for line in self.bytes.chunks_exact_mut(stride).take(painted_rows) {
            line[..painted_row].fill(0);
        }
        let row = width * BYTES_PER_PIXEL;
        if row == 0 {
            self.painted = (0, 0);
            return;
        }
        for (source, line) in argb
            .chunks_exact(row)
            .zip(self.bytes.chunks_exact_mut(stride))
        {
            line[..row].copy_from_slice(source);
        }
        self.painted = (argb.len() / row, row);
    }
}
