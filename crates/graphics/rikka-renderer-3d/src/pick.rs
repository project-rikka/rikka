use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::error::Result;
use crate::pipeline::Compute;
use crate::state::Renderer3dState;
use rikka_gpu::core::barrier::MemoryAccess;
use rikka_gpu::core::buffer::GpuBuffer;
use rikka_gpu::core::compute::groups;
use rikka_gpu::core::memory::Placement;
use rikka_gpu::core::runtime::GpuRuntime;
use rikka_gpu::reclaim::{Deferred, Reclaimer};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PickPush {
    out_ids: u64,
    origin: [i32; 2],
    size: [i32; 2],
    identity: u32,
    _pad: u32,
}

const PICK_GROUP: u32 = 8;

const SMALLEST_BUFFER: u64 = 256;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PickArea {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl PickArea {
    pub(crate) fn new(x: i32, y: i32, width: u32, height: u32, extent: vk::Extent2D) -> Self {
        let clamp = |start: i32, length: u32, limit: u32| {
            let start = i64::from(start);
            let first = start.clamp(0, i64::from(limit));
            let end = (start + i64::from(length)).clamp(first, i64::from(limit));
            (first as u32, (end - first) as u32)
        };
        let (x, width) = clamp(x, width, extent.width);
        let (y, height) = clamp(y, height, extent.height);
        Self {
            x,
            y,
            width,
            height,
        }
    }

    fn pixels(self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    pub fn contains(self, x: i32, y: i32) -> bool {
        let (x, y) = (i64::from(x), i64::from(y));
        x >= i64::from(self.x)
            && y >= i64::from(self.y)
            && x < i64::from(self.x) + i64::from(self.width)
            && y < i64::from(self.y) + i64::from(self.height)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Idle,
    Reading(u64),
    Claimed,
}

pub struct Pick {
    buffer: Option<Deferred<GpuBuffer>>,
    area: PickArea,
    state: State,
    reclaimer: Reclaimer,
}

impl Pick {
    pub(crate) fn new(reclaimer: Reclaimer) -> Self {
        Self {
            buffer: None,
            area: PickArea::default(),
            state: State::Idle,
            reclaimer,
        }
    }

    #[must_use]
    pub fn in_flight(&self) -> bool {
        matches!(self.state, State::Reading(_))
    }

    #[must_use]
    pub fn area(&self) -> PickArea {
        self.area
    }

    #[must_use]
    pub fn at(&self, x: i32, y: i32) -> Option<u32> {
        if self.state != State::Claimed || !self.area.contains(x, y) {
            return None;
        }
        let column = (i64::from(x) - i64::from(self.area.x)) as usize;
        let row = (i64::from(y) - i64::from(self.area.y)) as usize;
        self.raw()
            .get(row * self.area.width as usize + column)
            .and_then(|&id| id.checked_sub(1))
    }

    pub fn ids(&self) -> impl Iterator<Item = Option<u32>> + '_ {
        let claimed = match self.state {
            State::Claimed => self.raw(),
            _ => &[],
        };
        claimed.iter().map(|&id| id.checked_sub(1))
    }

    fn raw(&self) -> &[u32] {
        let count = self.area.pixels() as usize;
        match (&self.buffer, count) {
            (_, 0) | (None, _) => &[],
            (Some(buffer), _) => buffer
                .mapped_slice()
                .map_or(&[], |mapped| &bytemuck::cast_slice(mapped)[..count]),
        }
    }

    pub(crate) fn claim(&mut self, completed: u64) -> bool {
        match self.state {
            State::Reading(ready_at) if ready_at <= completed => {
                self.state = State::Claimed;
                true
            }
            _ => false,
        }
    }

    pub(crate) fn ready_at(&self) -> Option<u64> {
        match self.state {
            State::Reading(ready_at) => Some(ready_at),
            _ => None,
        }
    }
}

impl std::fmt::Debug for Pick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pick")
            .field("area", &self.area)
            .field("in_flight", &self.in_flight())
            .finish_non_exhaustive()
    }
}

pub(crate) fn start(
    gpu: &mut GpuRuntime,
    mesh: &mut Renderer3dState,
    identity: u32,
    area: PickArea,
    into: &mut Pick,
) -> Result<()> {
    into.area = area;
    if area.pixels() == 0 {
        into.state = State::Reading(0);
        return Ok(());
    }

    let bytes = (area.pixels() * 4).max(SMALLEST_BUFFER);
    let busy = matches!(into.state, State::Reading(ready_at) if ready_at > gpu.completed_value());
    if busy
        || into
            .buffer
            .as_ref()
            .is_none_or(|buffer| buffer.size() < bytes)
    {
        into.buffer = Some(into.reclaimer.defer(GpuBuffer::addressable(
            mesh.device().clone(),
            "rikka.three.pick",
            bytes,
            vk::BufferUsageFlags::STORAGE_BUFFER,
            Placement::Readback,
        )?));
    }
    let out_ids = into
        .buffer
        .as_ref()
        .expect("buffer exists")
        .device_address();

    let pipeline = mesh.pipelines.compute(Compute::Pick)?;
    let layout = mesh.pipelines.layout();
    let push = PickPush {
        out_ids,
        origin: [area.x as i32, area.y as i32],
        size: [area.width as i32, area.height as i32],
        identity,
        _pad: 0,
    };

    let mut ctx = gpu.begin_frame()?;
    ctx.begin_pass("rikka.three.pick");
    let sets = [ctx.resources.set()];
    unsafe {
        let device = ctx.device.handle();
        device.cmd_bind_pipeline(ctx.cmd, vk::PipelineBindPoint::COMPUTE, pipeline);
        device.cmd_bind_descriptor_sets(
            ctx.cmd,
            vk::PipelineBindPoint::COMPUTE,
            layout,
            0,
            &sets,
            &[],
        );
        device.cmd_push_constants(
            ctx.cmd,
            layout,
            vk::ShaderStageFlags::VERTEX
                | vk::ShaderStageFlags::FRAGMENT
                | vk::ShaderStageFlags::COMPUTE,
            0,
            bytemuck::bytes_of(&push),
        );
        device.cmd_dispatch(
            ctx.cmd,
            groups(area.width, PICK_GROUP),
            groups(area.height, PICK_GROUP),
            1,
        );
    }
    ctx.memory_barrier(
        MemoryAccess::new(
            vk::PipelineStageFlags2::COMPUTE_SHADER,
            vk::AccessFlags2::SHADER_STORAGE_WRITE,
        ),
        MemoryAccess::new(vk::PipelineStageFlags2::HOST, vk::AccessFlags2::HOST_READ),
    );
    ctx.end_pass();
    let ready_at = gpu.submit(&[], &[])?;
    into.state = State::Reading(ready_at);
    Ok(())
}
