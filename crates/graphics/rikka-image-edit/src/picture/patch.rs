use bytemuck::{Pod, Zeroable};

use crate::error::{Error, Result};
use crate::ops::passes::CopyPush;
use crate::picture::Picture;
use crate::rewrite::rewritable;
use crate::shader::Shader;
use crate::work::{Io, PushHeader, Work};

const COPY: u32 = 0;
const PASTE: u32 = 4;
const NONE: u32 = u32::MAX;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DifferPush {
    head: PushHeader,
    bounds: u64,
}

impl Picture {
    pub fn changed_area(&self, earlier: &Picture) -> Result<Option<[u32; 4]>> {
        let (now, then) = (&self.frame().plane, &earlier.frame().plane);
        if now.extent() != then.extent() || now.format() != then.format() {
            return Ok(Some([0, 0, self.width(), self.height()]));
        }
        let differing = now
            .areas()
            .enumerate()
            .filter(|(index, _)| !now.shares_tile(*index, then))
            .map(|(_, area)| area)
            .reduce(enclosing);
        let Some([left, top, across, down]) = differing else {
            return Ok(None);
        };
        let mut work = Work::new(self.imaging());
        let bounds = work.zeroed(16)?;
        let origin = [left, top].map(|value| i32::try_from(value).unwrap_or(i32::MAX));
        let push = DifferPush {
            head: PushHeader::new(then, now).with_origin(origin),
            bounds: bounds.address(),
        };
        let groups = [across.div_ceil(16), down.div_ceil(16), 1];
        let io = Io::input(then).read(now).write_buffer(&bounds);
        let cost = f64::from(across) * f64::from(down);
        work.groups(Shader::Differ, push, io, groups, cost)?;
        let download = work.batch().download_buffer(&bounds);
        work.submit()?;
        let words: Vec<u32> = bytemuck::pod_collect_to_vec(&download.wait()?);
        let [left, top, right, bottom] = [NONE - words[0], NONE - words[1], words[2], words[3]];
        Ok((right > left && bottom > top).then(|| [left, top, right - left, bottom - top]))
    }

    pub fn piece(&self, area: [u32; 4]) -> Result<Picture> {
        let frame = self.frame();
        let [left, top, across, down] = area;
        let [width, height] = frame.extent();
        let inside = across > 0
            && down > 0
            && left.checked_add(across).is_some_and(|right| right <= width)
            && top.checked_add(down).is_some_and(|bottom| bottom <= height);
        if !inside {
            return Err(Error::Invalid(format!(
                "{across}x{down} at {left},{top} is outside the {width}x{height} picture"
            )));
        }
        let mut work = Work::new(self.imaging());
        let out = work.plane([across, down], frame.plane.format())?;
        let push = CopyPush {
            head: PushHeader::new(&frame.plane, &out).mode(COPY),
            from: [left, top].map(|value| i32::try_from(value).unwrap_or(i32::MAX)),
            quarter: 0,
            mirror: 0,
            fill: [0.0; 4],
        };
        work.pass(Shader::Copy, push, Io::new(&frame.plane, &out), 1.0)?;
        work.finish()?;
        Ok(Picture::from_frame(self.imaging(), frame.with_plane(out)))
    }

    pub fn patched(&self, patch: &Picture, at: [u32; 2]) -> Result<Picture> {
        let (frame, piece) = (self.frame(), &patch.frame().plane);
        let [width, height] = frame.extent();
        let [across, down] = piece.extent();
        let inside = at[0]
            .checked_add(across)
            .is_some_and(|right| right <= width)
            && at[1]
                .checked_add(down)
                .is_some_and(|bottom| bottom <= height);
        if !inside || piece.format() != frame.plane.format() {
            return Err(Error::Invalid(format!(
                "{across}x{down} patch at {},{} does not fit the {width}x{height} picture",
                at[0], at[1]
            )));
        }
        let mut work = Work::new(self.imaging());
        let area = [at[0], at[1], across, down];
        let out = rewritable(&mut work, &frame.plane, frame.plane.format(), area)?;
        let origin = at.map(|value| i32::try_from(value).unwrap_or(i32::MAX));
        let pasted = CopyPush {
            head: PushHeader::new(piece, &out).mode(PASTE).with_origin(origin),
            from: origin.map(|value| -value),
            quarter: 0,
            mirror: 0,
            fill: [0.0; 4],
        };
        let groups = [across.div_ceil(16), down.div_ceil(16), 1];
        let cost = f64::from(across) * f64::from(down);
        let io = Io::new(piece, &out).limit_writes(area);
        work.groups(Shader::Copy, pasted, io, groups, cost)?;
        work.finish()?;
        Ok(Picture::from_frame(self.imaging(), frame.with_plane(out)))
    }
}

fn enclosing(one: [u32; 4], other: [u32; 4]) -> [u32; 4] {
    let left = one[0].min(other[0]);
    let top = one[1].min(other[1]);
    let right = (one[0] + one[2]).max(other[0] + other[2]);
    let bottom = (one[1] + one[3]).max(other[1] + other[3]);
    [left, top, right - left, bottom - top]
}
