use bytemuck::{Pod, Zeroable};
use rikka_gpu::{ComputedCopy, ComputedFrame, ImageFormat};

use crate::error::{Error, Result};
use crate::picture::Picture;
use crate::plane::Plane;
use crate::shader::Shader;
use crate::work::{Io, PushHeader, Work};
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct ConvertPush {
    pub(crate) head: PushHeader,
    pub(crate) data: u64,
    pub(crate) cube: u32,
    pub(crate) premultiply: u32,
    pub(crate) from: [i32; 2],
}

impl Picture {
    pub fn shown(&self) -> Result<ComputedFrame> {
        Ok(self.show(None)?.frame().clone())
    }

    pub fn shown_area(&self, origin: [u32; 2], extent: [u32; 2]) -> Result<ComputedFrame> {
        let inside = extent[0] > 0
            && extent[1] > 0
            && origin[0].saturating_add(extent[0]) <= self.width()
            && origin[1].saturating_add(extent[1]) <= self.height();
        if !inside {
            return Err(Error::Invalid(format!(
                "{}×{} at {},{} is outside the {}×{} picture",
                extent[0],
                extent[1],
                origin[0],
                origin[1],
                self.width(),
                self.height()
            )));
        }
        let frame = self.frame();
        let mut work = Work::new(self.imaging());
        let out = work.plane(extent, ImageFormat::Rgba8)?;
        let display = frame.space.display(&work)?;
        let push = ConvertPush {
            head: PushHeader::new(&frame.plane, &out)
                .mode(display.kind as u32)
                .clamped(true),
            data: display.data.address(),
            cube: display.cube,
            premultiply: 1,
            from: [origin[0] as i32, origin[1] as i32],
        };
        work.pass(
            Shader::Convert,
            push,
            Io::new(&frame.plane, &out).read_buffer(&display.data),
            1.0,
        )?;
        let ticket = work.finish()?;
        shown_parts(&out, &ticket)
    }
}

pub(crate) fn shown_parts(out: &Plane, ticket: &rikka_gpu::Ticket) -> Result<ComputedFrame> {
    let parts = out
        .tiles_with_areas()
        .map(|([left, top, width, height], tile)| {
            let part = ComputedCopy::part(tile, ticket, [0, 0], [width, height])?;
            Ok(([left, top], part))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ComputedFrame::new(out.width(), out.height(), parts)?)
}

crate::work::push_header!(ConvertPush);
