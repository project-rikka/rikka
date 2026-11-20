use std::sync::Arc;

use ash::vk;
use bytemuck::Pod;

use crate::build::DeformAddresses;
use crate::editor::GpuEditor;
use crate::error::Result;
use crate::fluid::{GpuFluid, GpuFluidLayer, GpuFluidMaterial, GpuFluidView};
use crate::gpu::{
    GpuBucket, GpuCluster, GpuDrawCommand, GpuFrameRefs, GpuInstance, GpuLight, GpuShadowSlot,
    GpuView, element_address,
};
use crate::look::gpu::{GpuFigure, GpuLook};
use crate::overlay::{ImmediateBases, OverlayFrame, Overlays};
use crate::pool::traced_input;
use crate::space::{EmitterFrame, PARTICLE_BYTES};
use crate::vertex::{GpuAttribute, GpuPosition};
use rikka_gpu::core::buffer::GpuBuffer;
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame::MAX_FRAMES_IN_FLIGHT;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::memory::Placement;

pub(crate) const CLUSTER_MAX_LIGHTS: u32 = 128;

const CLUSTER_INDEX_BYTES: u64 = 64 * 1024 * 1024;

pub(crate) fn cluster_stride(cluster_count: u32, local_lights: u32) -> u32 {
    if cluster_count == 0 || local_lights == 0 {
        return 0;
    }
    let affordable = CLUSTER_INDEX_BYTES / (u64::from(cluster_count) * 4);
    let affordable = affordable.clamp(1, u64::from(CLUSTER_MAX_LIGHTS)) as u32;
    local_lights.min(affordable)
}

const INITIAL_BUCKETS: u32 = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Writer {
    ByHost,
    ByDevice,
}

impl Writer {
    fn usage(self) -> vk::BufferUsageFlags {
        match self {
            Self::ByHost => vk::BufferUsageFlags::STORAGE_BUFFER,
            Self::ByDevice => {
                vk::BufferUsageFlags::STORAGE_BUFFER
                    | vk::BufferUsageFlags::INDIRECT_BUFFER
                    | vk::BufferUsageFlags::TRANSFER_DST
            }
        }
    }

    fn location(self) -> Placement {
        match self {
            Self::ByHost => Placement::Upload,
            Self::ByDevice => Placement::Device,
        }
    }
}

struct FrameBuffer {
    buffer: GpuBuffer,
    capacity: u64,
    name: &'static str,
    written: Writer,
    usage: vk::BufferUsageFlags,
}

const MIN_CAPACITY: u64 = 256;

impl FrameBuffer {
    fn new(
        device: &Arc<RenderDevice>,
        name: &'static str,
        written: Writer,
        capacity: u64,
    ) -> Result<Self> {
        Self::with_usage(
            device,
            name,
            written,
            capacity,
            vk::BufferUsageFlags::empty(),
        )
    }

    fn with_usage(
        device: &Arc<RenderDevice>,
        name: &'static str,
        written: Writer,
        capacity: u64,
        extra: vk::BufferUsageFlags,
    ) -> Result<Self> {
        let capacity = capacity.max(MIN_CAPACITY);
        let usage = written.usage() | extra;
        Ok(Self {
            buffer: GpuBuffer::addressable(
                device.clone(),
                name,
                capacity,
                usage,
                written.location(),
            )?,
            capacity,
            name,
            written,
            usage,
        })
    }

    fn ensure(&mut self, ctx: &mut GpuFrameCtx<'_>, needed: u64) -> Result<()> {
        if needed <= self.capacity {
            return Ok(());
        }
        let capacity = needed.next_power_of_two();
        let replacement = GpuBuffer::addressable(
            ctx.device.clone(),
            self.name,
            capacity,
            self.usage,
            self.written.location(),
        )?;
        let previous = std::mem::replace(&mut self.buffer, replacement);
        ctx.retire.defer(ctx.frame_value, Box::new(previous));
        self.capacity = capacity;
        Ok(())
    }

    fn write<T: Pod>(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        items: &[T],
    ) -> Result<vk::DeviceAddress> {
        let bytes: &[u8] = bytemuck::cast_slice(items);
        self.ensure(ctx, bytes.len() as u64)?;
        if !bytes.is_empty() {
            self.buffer
                .mapped_slice_mut()
                .expect("frame data is host-visible")[..bytes.len()]
                .copy_from_slice(bytes);
        }
        Ok(self.address())
    }

    fn address(&self) -> vk::DeviceAddress {
        self.buffer.device_address()
    }

    fn handle(&self) -> vk::Buffer {
        self.buffer.handle()
    }
}

struct HostSlot {
    view: FrameBuffer,
    instances: FrameBuffer,
    lights: FrameBuffer,
    shadows: FrameBuffer,
    bones: FrameBuffer,
    morph_weights: FrameBuffer,
    refs: FrameBuffer,
    buckets: FrameBuffer,
    look: FrameBuffer,
    figures: FrameBuffer,
    fluid: FrameBuffer,
    fluid_layers: FrameBuffer,
    fluid_materials: FrameBuffer,
    fluid_views: FrameBuffer,
    effects: FrameBuffer,
    previous_transforms: FrameBuffer,
    emitters: FrameBuffer,
    emitter_keys: FrameBuffer,
    overlay_view: FrameBuffer,
    editor: FrameBuffer,
    overlay_refs: FrameBuffer,
    overlay_positions: FrameBuffer,
    overlay_colors: FrameBuffer,
    overlay_indices: FrameBuffer,
    overlay_batches: FrameBuffer,
}

impl HostSlot {
    fn new(device: &Arc<RenderDevice>) -> Result<Self> {
        let host = |name, capacity| FrameBuffer::new(device, name, Writer::ByHost, capacity);
        Ok(Self {
            view: host("rikka.three.view", 1024)?,
            instances: host("rikka.three.instances", 64 * 1024)?,
            lights: host("rikka.three.lights", 8 * 1024)?,
            shadows: host("rikka.three.shadows", 8 * 1024)?,
            bones: host("rikka.three.bones", 16 * 1024)?,
            morph_weights: host("rikka.three.morph_weights", 4 * 1024)?,
            refs: host("rikka.three.refs", 256)?,
            buckets: host("rikka.three.buckets", 1024)?,
            look: host("rikka.three.look", 1024)?,
            figures: host("rikka.three.figures", 4 * 1024)?,
            fluid: host("rikka.three.fluid", 256)?,
            fluid_layers: host("rikka.three.fluid_layers", 256)?,
            fluid_materials: host("rikka.three.fluid_materials", 512)?,
            fluid_views: host("rikka.three.fluid_views", 1024)?,
            effects: host("rikka.three.effects", 1024)?,
            previous_transforms: host("rikka.three.previous_transforms", 16 * 1024)?,
            emitters: host("rikka.three.emitters", 4 * 1024)?,
            emitter_keys: host("rikka.three.emitter_keys", 1024)?,
            overlay_view: host("rikka.three.overlay_view", 1024)?,
            editor: host("rikka.three.editor", 1024)?,
            overlay_refs: host("rikka.three.overlay_refs", 256)?,
            overlay_positions: host("rikka.three.overlay_positions", 4 * 1024)?,
            overlay_colors: host("rikka.three.overlay_colors", 1024)?,
            overlay_indices: host("rikka.three.overlay_indices", 1024)?,
            overlay_batches: host("rikka.three.overlay_batches", 4 * 1024)?,
        })
    }
}

struct DrawScratch {
    clusters: FrameBuffer,
    cluster_indices: FrameBuffer,
    counters: FrameBuffer,
    commands: FrameBuffer,
    late_instances: FrameBuffer,
    particles: FrameBuffer,
}

impl DrawScratch {
    fn new(device: &Arc<RenderDevice>) -> Result<Self> {
        let scratch = |name, capacity| FrameBuffer::new(device, name, Writer::ByDevice, capacity);
        Ok(Self {
            clusters: scratch("rikka.three.clusters", 64 * 1024)?,
            cluster_indices: scratch("rikka.three.cluster_indices", 256 * 1024)?,
            counters: scratch(
                "rikka.three.counters",
                u64::from(COUNTER_BUCKETS + INITIAL_BUCKETS) * 4,
            )?,
            commands: scratch("rikka.three.commands", 256 * 1024)?,
            late_instances: scratch("rikka.three.late", 64 * 1024)?,
            particles: scratch("rikka.three.particles", 64 * 1024)?,
        })
    }
}

struct FrameSlot {
    host: HostSlot,
    draws: DrawScratch,
}

struct Deform {
    positions: FrameBuffer,
    attributes: FrameBuffer,
}

impl Deform {
    fn new(device: &Arc<RenderDevice>) -> Result<Self> {
        Ok(Self {
            positions: FrameBuffer::with_usage(
                device,
                "rikka.three.deform_positions",
                Writer::ByDevice,
                256 * 1024,
                traced_input(device) | vk::BufferUsageFlags::TRANSFER_SRC,
            )?,
            attributes: FrameBuffer::new(
                device,
                "rikka.three.deform_attributes",
                Writer::ByDevice,
                256 * 1024,
            )?,
        })
    }
}

pub(crate) const COUNTER_LATE: u32 = 0;
pub(crate) const COUNTER_LATE_DISPATCH: u32 = 1;
pub(crate) const COUNTER_PYRAMID: u32 = 4;
pub(crate) const COUNTER_BUCKETS: u32 = 5;

pub(crate) struct FrameWrite<'a> {
    pub view: &'a GpuView,
    pub instances: &'a [GpuInstance],
    pub lights: &'a [GpuLight],
    pub shadows: &'a [GpuShadowSlot],
    pub bones: &'a [[f32; 16]],
    pub morph_weights: &'a [f32],
    pub materials: vk::DeviceAddress,
    pub blocks: vk::DeviceAddress,
    pub buckets: &'a [GpuBucket],
    pub profiles: vk::DeviceAddress,
    pub cluster_count: u32,
    pub cluster_stride: u32,
    pub look: &'a GpuLook,
    pub figures: &'a [GpuFigure],
    pub environment: vk::DeviceAddress,
    pub previous_positions: vk::DeviceAddress,
    pub fluid: vk::DeviceAddress,
    pub previous_transforms: &'a [[f32; 16]],
    pub overlay_view: Option<&'a GpuView>,
    pub editor: Option<&'a GpuEditor>,
}

pub(crate) struct FluidWrite<'a> {
    pub header: GpuFluid,
    pub layers: &'a [GpuFluidLayer],
    pub materials: &'a [GpuFluidMaterial],
    pub views: &'a [GpuFluidView],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FrameAddresses {
    pub refs: vk::DeviceAddress,
    pub counters: vk::DeviceAddress,
    pub counters_buffer: vk::Buffer,
    pub commands_buffer: vk::Buffer,
    pub deform_positions: vk::DeviceAddress,
    pub deform_attributes: vk::DeviceAddress,
    pub morph_weights: vk::DeviceAddress,
    pub effects: vk::DeviceAddress,
    pub emitters: EmitterAddresses,
    pub overlay_refs: vk::DeviceAddress,
    pub overlay_batches: vk::DeviceAddress,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct EmitterAddresses {
    pub emitters: vk::DeviceAddress,
    pub keys: vk::DeviceAddress,
    pub particles: vk::DeviceAddress,
}

pub(crate) struct FrameData {
    deform: Deform,
    slots: Vec<FrameSlot>,
    ltc: GpuBuffer,
}

fn ltc_table(device: &Arc<RenderDevice>) -> Result<GpuBuffer> {
    let table = rikka_ltc::GGX;
    let mut buffer = GpuBuffer::addressable(
        device.clone(),
        "rikka.three.ltc",
        table.len() as u64,
        vk::BufferUsageFlags::STORAGE_BUFFER,
        Placement::Upload,
    )?;
    buffer
        .mapped_slice_mut()
        .expect("the LTC table is host-visible")[..table.len()]
        .copy_from_slice(table);
    Ok(buffer)
}

impl FrameData {
    pub(crate) fn new(device: &Arc<RenderDevice>) -> Result<Self> {
        let slots = (0..MAX_FRAMES_IN_FLIGHT)
            .map(|_| {
                Ok(FrameSlot {
                    host: HostSlot::new(device)?,
                    draws: DrawScratch::new(device)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            deform: Deform::new(device)?,
            ltc: ltc_table(device)?,
            slots,
        })
    }

    pub(crate) fn deform_positions(&self) -> vk::Buffer {
        self.deform.positions.handle()
    }

    pub(crate) fn ensure_deform(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        vertices: u32,
    ) -> Result<DeformAddresses> {
        let deform = &mut self.deform;
        deform
            .positions
            .ensure(ctx, u64::from(vertices) * size_of::<GpuPosition>() as u64)?;
        deform
            .attributes
            .ensure(ctx, u64::from(vertices) * size_of::<GpuAttribute>() as u64)?;
        Ok(DeformAddresses {
            positions: deform.positions.address(),
            attributes: deform.attributes.address(),
        })
    }

    pub(crate) fn write_overlays(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        overlays: &Overlays,
        frame: &mut OverlayFrame,
    ) -> Result<vk::DeviceAddress> {
        if frame.is_empty() {
            return Ok(0);
        }
        let slot = self.slot(ctx);
        let bases = ImmediateBases {
            positions: slot.overlay_positions.write(ctx, &overlays.positions)?,
            colors: slot.overlay_colors.write(ctx, &overlays.colors)?,
            indices: slot.overlay_indices.write(ctx, &overlays.indices)?,
        };
        frame.place(bases);
        slot.overlay_batches.write(ctx, frame.records())
    }

    pub(crate) fn write_effects(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        blocks: &[u8],
    ) -> Result<vk::DeviceAddress> {
        self.slot(ctx).effects.write(ctx, blocks)
    }

    pub(crate) fn write_emitters(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        frame: &EmitterFrame,
    ) -> Result<EmitterAddresses> {
        if frame.is_empty() {
            return Ok(EmitterAddresses::default());
        }
        let FrameSlot { host, draws } = &mut self.slots[ctx.frame_slot];
        draws
            .particles
            .ensure(ctx, u64::from(frame.particles()) * PARTICLE_BYTES)?;
        Ok(EmitterAddresses {
            emitters: host.emitters.write(ctx, frame.emitters())?,
            keys: host.emitter_keys.write(ctx, frame.keys())?,
            particles: draws.particles.address(),
        })
    }

    pub(crate) fn write_fluid(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        write: &FluidWrite<'_>,
    ) -> Result<vk::DeviceAddress> {
        let slot = self.slot(ctx);
        let header = GpuFluid {
            layers: slot.fluid_layers.write(ctx, write.layers)?,
            materials: slot.fluid_materials.write(ctx, write.materials)?,
            views: slot.fluid_views.write(ctx, write.views)?,
            ..write.header
        };
        slot.fluid.write(ctx, std::slice::from_ref(&header))
    }

    fn slot(&mut self, ctx: &GpuFrameCtx<'_>) -> &mut HostSlot {
        &mut self.slots[ctx.frame_slot].host
    }

    pub(crate) fn write(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        write: &FrameWrite<'_>,
    ) -> Result<FrameAddresses> {
        let commands = write
            .buckets
            .iter()
            .map(|bucket| u64::from(bucket.capacity))
            .sum::<u64>();
        let counters = u64::from(COUNTER_BUCKETS) + write.buckets.len() as u64;
        let indices = u64::from(write.cluster_count) * u64::from(write.cluster_stride);

        let Self { deform, slots, ltc } = self;
        let FrameSlot {
            host: slot,
            draws: scratch,
        } = &mut slots[ctx.frame_slot];
        scratch.clusters.ensure(
            ctx,
            u64::from(write.cluster_count) * size_of::<GpuCluster>() as u64,
        )?;
        scratch.cluster_indices.ensure(ctx, indices * 4)?;
        scratch
            .commands
            .ensure(ctx, commands * size_of::<GpuDrawCommand>() as u64)?;
        scratch.counters.ensure(ctx, counters * 4)?;
        scratch
            .late_instances
            .ensure(ctx, write.instances.len() as u64 * 4)?;

        let view_address = slot.view.write(ctx, std::slice::from_ref(write.view))?;
        let instances = slot.instances.write(ctx, write.instances)?;
        let lights = slot.lights.write(ctx, write.lights)?;
        let shadows = slot.shadows.write(ctx, write.shadows)?;
        let bones = slot.bones.write(ctx, write.bones)?;
        let morph_weights = slot.morph_weights.write(ctx, write.morph_weights)?;
        let buckets = slot.buckets.write(ctx, write.buckets)?;
        let look = slot.look.write(ctx, std::slice::from_ref(write.look))?;
        let figures = slot.figures.write(ctx, write.figures)?;
        let previous_transforms = slot
            .previous_transforms
            .write(ctx, write.previous_transforms)?;
        let editor = match write.editor {
            Some(editor) => slot.editor.write(ctx, std::slice::from_ref(editor))?,
            None => 0,
        };

        let counters_address = scratch.counters.address();
        let refs = GpuFrameRefs {
            view: view_address,
            instances,
            materials: write.materials,
            lights,
            shadows,
            bones,
            clusters: scratch.clusters.address(),
            cluster_indices: scratch.cluster_indices.address(),
            buckets,
            draw_commands: scratch.commands.address(),
            draw_counts: element_address::<u32>(counters_address, COUNTER_BUCKETS),
            late_instances: scratch.late_instances.address(),
            late_counter: element_address::<u32>(counters_address, COUNTER_LATE),
            look,
            figures,
            environment: write.environment,
            previous_positions: write.previous_positions,
            fluid: write.fluid,
            blocks: write.blocks,
            previous_transforms,
            ltc: ltc.device_address(),
            profiles: write.profiles,
            editor,
        };
        let refs_address = slot.refs.write(ctx, std::slice::from_ref(&refs))?;
        let overlay_refs = match write.overlay_view {
            Some(view) => {
                let view = slot.overlay_view.write(ctx, std::slice::from_ref(view))?;
                let refs = GpuFrameRefs { view, ..refs };
                slot.overlay_refs.write(ctx, std::slice::from_ref(&refs))?
            }
            None => 0,
        };

        let counters_buffer = scratch.counters.handle();
        let dispatch_rows = u64::from(COUNTER_LATE_DISPATCH + 1) * 4;
        let dispatch_end = u64::from(COUNTER_PYRAMID) * 4;
        unsafe {
            let device = ctx.device.handle();
            device.cmd_fill_buffer(ctx.cmd, counters_buffer, 0, dispatch_rows, 0);
            device.cmd_fill_buffer(
                ctx.cmd,
                counters_buffer,
                dispatch_rows,
                dispatch_end - dispatch_rows,
                1,
            );
            device.cmd_fill_buffer(
                ctx.cmd,
                counters_buffer,
                dispatch_end,
                counters * 4 - dispatch_end,
                0,
            );
        }

        Ok(FrameAddresses {
            refs: refs_address,
            counters: counters_address,
            counters_buffer,
            commands_buffer: scratch.commands.handle(),
            deform_positions: deform.positions.address(),
            deform_attributes: deform.attributes.address(),
            morph_weights,
            effects: 0,
            emitters: EmitterAddresses::default(),
            overlay_refs,
            overlay_batches: 0,
        })
    }
}
