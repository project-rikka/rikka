use bytemuck::{Pod, Zeroable};
use rikka_gpu::{Batch, ComputeBuffer, Dispatch, ImageFormat, Ticket};

use crate::error::Result;
use crate::imaging::Imaging;
use crate::plane::{Plane, PlaneRef, intersects, saturating_i32};
use crate::shader::Shader;

pub(crate) const CLAMP: u32 = 1;

const LOCAL: u32 = 16;

const BAND_COST: f64 = 1.5e9;

const FLUSH_COST: f64 = 3.0e9;

const MEMORY_BUDGET: u64 = 1 << 30;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct PushHeader {
    src: PlaneRef,
    dst: PlaneRef,
    origin: [i32; 2],
    flags: u32,
    mode: u32,
    window: [i32; 2],
    pad: [u32; 2],
}

impl PushHeader {
    pub(crate) fn new(src: &Plane, dst: &Plane) -> PushHeader {
        PushHeader {
            src: src.reference(),
            dst: dst.reference(),
            origin: [0, 0],
            flags: 0,
            mode: 0,
            window: [0, 0],
            pad: [0, 0],
        }
    }

    pub(crate) fn window(mut self, window: [i32; 2]) -> PushHeader {
        self.window = window;
        self
    }

    pub(crate) fn mode(mut self, mode: u32) -> PushHeader {
        self.mode = mode;
        self
    }

    pub(crate) fn with_origin(mut self, origin: [i32; 2]) -> PushHeader {
        self.origin = origin;
        self
    }

    pub(crate) fn clamped(mut self, clamp: bool) -> PushHeader {
        self.flags = match clamp {
            true => self.flags | CLAMP,
            false => self.flags & !CLAMP,
        };
        self
    }
}

pub(crate) trait HasPushHeader: Pod {
    fn head(&mut self) -> &mut PushHeader;
}

impl HasPushHeader for PushHeader {
    fn head(&mut self) -> &mut PushHeader {
        self
    }
}

macro_rules! push_header {
    ($($push:ty),+ $(,)?) => {
        $(impl $crate::work::HasPushHeader for $push {
            fn head(&mut self) -> &mut $crate::work::PushHeader {
                &mut self.head
            }
        })+
    };
}

pub(crate) use push_header;

#[derive(Default)]
pub(crate) struct Io<'a> {
    reads: Vec<&'a Plane>,
    writes: Vec<&'a Plane>,
    buffers: Vec<(&'a ComputeBuffer, bool)>,
    write_area: Option<[u32; 4]>,
}

impl<'a> Io<'a> {
    pub(crate) fn new(src: &'a Plane, dst: &'a Plane) -> Io<'a> {
        Io {
            reads: vec![src],
            writes: vec![dst],
            ..Io::default()
        }
    }

    pub(crate) fn input(src: &'a Plane) -> Io<'a> {
        Io {
            reads: vec![src],
            ..Io::default()
        }
    }

    pub(crate) fn output(dst: &'a Plane) -> Io<'a> {
        Io {
            writes: vec![dst],
            ..Io::default()
        }
    }

    pub(crate) fn read(mut self, plane: &'a Plane) -> Io<'a> {
        self.reads.push(plane);
        self
    }

    pub(crate) fn read_buffer(mut self, buffer: &'a ComputeBuffer) -> Io<'a> {
        self.buffers.push((buffer, false));
        self
    }

    pub(crate) fn write_buffer(mut self, buffer: &'a ComputeBuffer) -> Io<'a> {
        self.buffers.push((buffer, true));
        self
    }

    pub(crate) fn limit_writes(mut self, area: [u32; 4]) -> Io<'a> {
        self.write_area = Some(area);
        self
    }

    fn dispatch(&self, groups: [u32; 3], push: &[u8]) -> Dispatch {
        let mut dispatch = Dispatch::new(groups).push(push);
        for plane in &self.reads {
            for tile in plane.tiles() {
                dispatch = dispatch.reads(tile);
            }
        }
        for plane in &self.writes {
            for (area, tile) in plane.areas().zip(plane.tiles()) {
                if self.write_area.is_none_or(|limit| intersects(limit, area)) {
                    dispatch = dispatch.writes(tile);
                }
            }
        }
        for (buffer, written) in &self.buffers {
            dispatch = match written {
                true => dispatch.writes_buffer(buffer),
                false => dispatch.reads_buffer(buffer),
            };
        }
        dispatch
    }
}

pub(crate) struct Work {
    imaging: Imaging,
    batch: Batch,
    pending_cost: f64,
    allocated_bytes: u64,
    pending: Option<Ticket>,
    canvas: Option<[u32; 2]>,
    window: [i32; 2],
}

impl Work {
    pub(crate) fn new(imaging: &Imaging) -> Work {
        Work {
            imaging: imaging.clone(),
            batch: imaging.compute().batch(),
            pending_cost: 0.0,
            allocated_bytes: 0,
            pending: None,
            canvas: None,
            window: [0, 0],
        }
    }

    pub(crate) fn windowed_on(imaging: &Imaging, canvas: [u32; 2], window: [i32; 2]) -> Work {
        let mut work = Work::new(imaging);
        work.canvas = Some(canvas);
        work.window = window;
        work
    }

    pub(crate) fn canvas_or(&self, extent: [u32; 2]) -> [u32; 2] {
        self.canvas.unwrap_or(extent)
    }

    pub(crate) fn window(&self) -> [i32; 2] {
        self.window
    }

    pub(crate) fn plane_like(&mut self, like: &Plane, format: ImageFormat) -> Result<Plane> {
        self.plane(like.extent(), format)
    }

    pub(crate) fn imaging(&self) -> &Imaging {
        &self.imaging
    }

    pub(crate) fn plane(&mut self, extent: [u32; 2], format: ImageFormat) -> Result<Plane> {
        let plane = Plane::new(&self.imaging, extent[0], extent[1], format)?;
        self.track_allocation(plane.bytes())?;
        Ok(plane)
    }

    pub(crate) fn derived(
        &mut self,
        source: &Plane,
        format: ImageFormat,
        is_new_tile: impl Fn(usize, [u32; 4]) -> bool,
    ) -> Result<Plane> {
        let plane = Plane::derived(&self.imaging, source, format, is_new_tile)?;
        let per_pixel = u64::from(format.bytes_per_pixel());
        let allocated: u64 = plane
            .areas()
            .enumerate()
            .filter(|(index, _)| !plane.shares_tile(*index, source))
            .map(|(_, [_, _, width, height])| u64::from(width) * u64::from(height) * per_pixel)
            .sum();
        self.track_allocation(allocated)?;
        Ok(plane)
    }

    fn track_allocation(&mut self, bytes: u64) -> Result<()> {
        self.allocated_bytes += bytes;
        if self.allocated_bytes >= MEMORY_BUDGET {
            if let Some(pending) = self.pending.take() {
                pending.wait()?;
            }
            self.pending = Some(self.submit()?);
        }
        Ok(())
    }

    pub(crate) fn constants<T: Pod>(&self, values: &[T]) -> Result<ComputeBuffer> {
        let bytes = match values.is_empty() {
            true => &[0; 4][..],
            false => bytemuck::cast_slice(values),
        };
        Ok(self.imaging.compute().constants(bytes)?)
    }

    pub(crate) fn zeroed(&mut self, bytes: u64) -> Result<ComputeBuffer> {
        let buffer = self.imaging.compute().buffer(bytes.max(4))?;
        self.batch.fill(&buffer, 0);
        Ok(buffer)
    }

    pub(crate) fn pass<P: HasPushHeader>(
        &mut self,
        shader: Shader,
        push: P,
        io: Io<'_>,
        cost: f64,
    ) -> Result<()> {
        let extent = io
            .writes
            .first()
            .map(|plane| plane.extent())
            .expect("pass writes at least one plane");
        self.pass_over(shader, push, io, extent, cost)
    }

    pub(crate) fn pass_over<P: HasPushHeader>(
        &mut self,
        shader: Shader,
        push: P,
        io: Io<'_>,
        [width, height]: [u32; 2],
        cost: f64,
    ) -> Result<()> {
        self.pass_at(shader, push, io, [0, 0, width, height], cost)
    }

    pub(crate) fn pass_at<P: HasPushHeader>(
        &mut self,
        shader: Shader,
        mut push: P,
        io: Io<'_>,
        [left, top, width, height]: [u32; 4],
        cost: f64,
    ) -> Result<()> {
        let kernel = self.imaging.kernel(shader)?;
        let total = f64::from(width) * f64::from(height) * cost.max(1.0);
        let bands = (total / BAND_COST).ceil().max(1.0) as u32;
        let rows = height.div_ceil(bands).next_multiple_of(LOCAL).max(LOCAL);
        let mut start = 0;
        while start < height {
            let span = rows.min(height - start);
            push.head().origin = [saturating_i32(left), saturating_i32(top + start)];
            push.head().window = self.window;
            let groups = [width.div_ceil(LOCAL), span.div_ceil(LOCAL), 1];
            self.batch
                .dispatch(&kernel, io.dispatch(groups, bytemuck::bytes_of(&push)));
            self.add_cost(f64::from(width) * f64::from(span) * cost.max(1.0))?;
            start += span;
        }
        Ok(())
    }

    pub(crate) fn groups<P: Pod>(
        &mut self,
        shader: Shader,
        push: P,
        io: Io<'_>,
        groups: [u32; 3],
        cost: f64,
    ) -> Result<()> {
        let kernel = self.imaging.kernel(shader)?;
        self.batch
            .dispatch(&kernel, io.dispatch(groups, bytemuck::bytes_of(&push)));
        self.add_cost(cost)
    }

    pub(crate) fn batch(&mut self) -> &mut Batch {
        &mut self.batch
    }

    fn add_cost(&mut self, cost: f64) -> Result<()> {
        self.pending_cost += cost;
        if self.pending_cost >= FLUSH_COST {
            self.submit()?;
        }
        Ok(())
    }

    pub(crate) fn submit(&mut self) -> Result<Ticket> {
        let batch = std::mem::replace(&mut self.batch, self.imaging.compute().batch());
        self.pending_cost = 0.0;
        self.allocated_bytes = 0;
        Ok(batch.submit()?)
    }

    pub(crate) fn finish(mut self) -> Result<Ticket> {
        self.submit()
    }
}
