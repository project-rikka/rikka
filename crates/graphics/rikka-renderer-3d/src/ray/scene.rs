use std::sync::Arc;

use ash::vk;
use bytemuck::{Pod, Zeroable};
use glam::Mat4;
use rikka_shader_programs::Variant;
use rustc_hash::FxHashMap;

use crate::build::BuiltFrame;
use crate::draw::Frame;
use crate::error::Result;
use crate::gpu::{GpuInstance, GpuLight, NONE_INDEX, instance_flags};
use crate::material::{MaterialPass, MaterialPool};
use crate::mesh::{IndexWidth, MeshId};
use crate::pool::MeshPool;
use crate::ray::commands::{Batch, Job, build};
use crate::ray::hooks::{HookJob, Hooks};
use crate::ray::structure::Structure;
use crate::vertex::GpuPosition;
use rikka_gpu::core::buffer::GpuBuffer;
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame::MAX_FRAMES_IN_FLIGHT;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::memory::Placement;

pub(crate) const MASK_SHADOW: u32 = 1 << 0;
pub(crate) const MASK_SEEN: u32 = 1 << 1;
const LINKED_MASKS: usize = 8;
const LAYER_SHIFT: u32 = 2;
const LAYER_BITS: u32 = 0x3f;

const REBUILD_AFTER: u32 = 30;
const DEFORMED_KEEP: u64 = 2;
const QUERIES: u32 = 64;
const MIN_INSTANCES: u32 = 64;
const MIN_PARTS: u64 = 256;
const BUDGET_SHARE: u64 = 4;
const FIRST_BUILD_TRIANGLES: u64 = 1 << 19;
const WHOLE_MESH: u32 = u32::MAX;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
struct GpuRayInstance {
    transform: [f32; 12],
    custom_and_mask: u32,
    offset_and_flags: u32,
    reference: u64,
}

const _: () = assert!(size_of::<GpuRayInstance>() == 64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Owner {
    figure: u32,
    object: u32,
    occurrence: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Shape {
    Static(MeshId),
    Deformed {
        mesh: MeshId,
        owner: Owner,
    },
    Hooked {
        mesh: MeshId,
        part: u32,
        owner: Owner,
    },
}

impl Shape {
    fn mesh(self) -> MeshId {
        match self {
            Self::Static(mesh) | Self::Deformed { mesh, .. } | Self::Hooked { mesh, .. } => mesh,
        }
    }

    fn moves(self) -> bool {
        !matches!(self, Self::Static(_))
    }

    fn flags(self, rebuild_after: u32) -> vk::BuildAccelerationStructureFlagsKHR {
        match (self.moves(), rebuild_after) {
            (false, _) => {
                vk::BuildAccelerationStructureFlagsKHR::ALLOW_COMPACTION
                    | vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE
            }
            (true, 0) => vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_BUILD,
            (true, _) => {
                vk::BuildAccelerationStructureFlagsKHR::ALLOW_UPDATE
                    | vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_BUILD
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Identity {
    mesh: MeshId,
    part: u32,
    figure: u32,
    object: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Compaction {
    Wanted,
    Pending { query: u32, value: u64 },
    Done,
    Never,
}

impl Compaction {
    fn query(self) -> Option<u32> {
        match self {
            Self::Pending { query, .. } => Some(query),
            Self::Wanted | Self::Done | Self::Never => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Progress {
    built: bool,
    source: vk::DeviceAddress,
    refits: u32,
    compaction: Compaction,
}

struct Bottom {
    structure: Structure,
    flags: vk::BuildAccelerationStructureFlagsKHR,
    build_scratch: u64,
    update_scratch: u64,
    progress: Progress,
    rebuild_after: u32,
    used: u64,
}

impl Bottom {
    fn refits(&self, source: vk::DeviceAddress) -> bool {
        let progress = &self.progress;
        progress.built
            && progress.source == source
            && progress.refits < self.rebuild_after
            && self
                .flags
                .contains(vk::BuildAccelerationStructureFlagsKHR::ALLOW_UPDATE)
    }
}

#[derive(Default)]
struct Staged {
    undo: Vec<(Shape, Progress)>,
    compacted: Vec<(Shape, Structure)>,
    recorded: bool,
}

struct Group {
    shape: Shape,
    source: vk::DeviceAddress,
    hook: u32,
    transform: Mat4,
    first_part: u32,
    parts: u32,
    drawn: u32,
    mask: u32,
    casters: u32,
    cutouts: bool,
    rebuild_after: u32,
    channels_all: u32,
    channels_any: u32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Linking {
    masks: [u32; LINKED_MASKS],
    count: usize,
    overflow: bool,
}

impl Linking {
    fn of(lights: &[GpuLight]) -> Self {
        let mut linking = Self::default();
        for light in lights.iter().filter(|light| light.shadow != NONE_INDEX) {
            if linking.masks[..linking.count].contains(&light.channels) {
                continue;
            }
            match linking.count < LINKED_MASKS {
                true => {
                    linking.masks[linking.count] = light.channels;
                    linking.count += 1;
                }
                false => linking.overflow = true,
            }
        }
        linking
    }

    fn reaches_all(&self, channels: u32) -> bool {
        match self.overflow {
            true => channels == u32::MAX,
            false => self.masks[..self.count]
                .iter()
                .all(|&mask| mask & channels != 0),
        }
    }

    fn reaches_any(&self, channels: u32) -> bool {
        self.overflow
            || self.masks[..self.count]
                .iter()
                .any(|&mask| mask & channels != 0)
    }
}

struct Slot {
    instances: GpuBuffer,
    instance_capacity: u32,
    parts: GpuBuffer,
    part_capacity: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RayFrame {
    pub scene: vk::DeviceAddress,
    pub parts: vk::DeviceAddress,
    pub indices16: vk::DeviceAddress,
    pub indices32: vk::DeviceAddress,
    pub hooks: vk::DeviceAddress,
}

pub(crate) struct RayScene {
    device: Arc<RenderDevice>,
    bottoms: FxHashMap<Shape, Bottom>,
    top: Option<Structure>,
    top_capacity: u32,
    top_scratch: u64,
    scratch: Option<GpuBuffer>,
    slots: Vec<Slot>,
    queries: vk::QueryPool,
    free_queries: Vec<u32>,
    groups: Vec<Group>,
    group_of: FxHashMap<(Shape, [u32; 16]), u32>,
    occurrences: FxHashMap<Identity, u32>,
    deformed: FxHashMap<(MeshId, vk::DeviceAddress), Owner>,
    parts: Vec<u32>,
    instances: Vec<GpuRayInstance>,
    frame: Batch,
    hooks: Hooks,
    first_built: u64,
    top_job: Option<(vk::DeviceAddress, u32, u64)>,
    retired: Vec<Structure>,
    staged: Staged,
    serial: u64,
    budget: u64,
    linking: Linking,
}

impl RayScene {
    pub(crate) fn new(device: Arc<RenderDevice>) -> Result<Self> {
        let slots = (0..MAX_FRAMES_IN_FLIGHT)
            .map(|_| {
                Ok(Slot {
                    instances: instance_buffer(&device, MIN_INSTANCES)?,
                    instance_capacity: MIN_INSTANCES,
                    parts: part_buffer(&device, MIN_PARTS)?,
                    part_capacity: MIN_PARTS,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let info = vk::QueryPoolCreateInfo::default()
            .query_type(vk::QueryType::ACCELERATION_STRUCTURE_COMPACTED_SIZE_KHR)
            .query_count(QUERIES);
        let queries = unsafe { device.handle().create_query_pool(&info, None)? };
        if device.resets_queries_on_host() {
            unsafe { device.handle().reset_query_pool(queries, 0, QUERIES) };
        }
        Ok(Self {
            budget: device_local_budget(&device) / BUDGET_SHARE,
            hooks: Hooks::new(device.clone()),
            first_built: 0,
            device,
            bottoms: FxHashMap::default(),
            top: None,
            top_capacity: 0,
            top_scratch: 0,
            scratch: None,
            slots,
            queries,
            free_queries: (0..QUERIES).rev().collect(),
            groups: Vec::new(),
            group_of: FxHashMap::default(),
            occurrences: FxHashMap::default(),
            deformed: FxHashMap::default(),
            parts: Vec::new(),
            instances: Vec::new(),
            frame: Batch::default(),
            top_job: None,
            retired: Vec::new(),
            staged: Staged::default(),
            serial: 0,
            linking: Linking::default(),
        })
    }

    pub(crate) fn hook_jobs(&self) -> &[HookJob] {
        self.hooks.jobs()
    }

    pub(crate) fn forget(&mut self, mesh: MeshId) {
        let shapes: smallvec::SmallVec<[Shape; 4]> = self
            .bottoms
            .keys()
            .copied()
            .filter(|shape| shape.mesh() == mesh)
            .collect();
        for shape in shapes {
            let Some(bottom) = self.bottoms.remove(&shape) else {
                continue;
            };
            self.release(bottom);
        }
    }

    fn release(&mut self, bottom: Bottom) {
        if let Some(query) = bottom.progress.compaction.query() {
            self.free_queries.push(query);
        }
        self.retired.push(bottom.structure);
    }

    pub(crate) fn prepare(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        frame: &Frame,
        built: &BuiltFrame,
        meshes: &MeshPool,
        materials: &MaterialPool,
    ) -> Result<RayFrame> {
        self.serial += 1;
        self.groups.clear();
        self.group_of.clear();
        self.occurrences.clear();
        self.deformed.clear();
        self.parts.clear();
        self.instances.clear();
        self.frame.clear();
        self.hooks.clear();
        self.first_built = 0;
        for structure in self.retired.drain(..) {
            ctx.retire.defer(ctx.frame_value, Box::new(structure));
        }

        self.advance(ctx)?;
        self.linking = Linking::of(&built.lights);
        self.gather(frame, built, meshes, materials);
        self.hooks.ensure(ctx)?;
        for group in &mut self.groups {
            if let Some(job) = group.hook.checked_sub(1) {
                group.source = self.hooks.positions(job);
            }
        }
        for at in 0..self.groups.len() {
            self.schedule(ctx, at, meshes)?;
        }
        self.retire_unused(ctx);
        let count = self.write_instances();
        self.schedule_top(count)?;
        self.ensure_scratch(ctx)?;
        let parts = self.write_slot(ctx, count)?;
        Ok(RayFrame {
            scene: self.top.as_ref().map_or(0, |top| top.address),
            parts,
            indices16: meshes.index_address(IndexWidth::U16),
            indices32: meshes.index_address(IndexWidth::U32),
            hooks: self.hooks.write_table(ctx)?,
        })
    }

    pub(crate) fn commit(&mut self) -> impl Iterator<Item = Structure> + '_ {
        if !self.staged.recorded {
            self.revert();
        }
        self.staged.recorded = false;
        self.staged.undo.clear();
        self.staged.compacted.drain(..).map(|(_, former)| former)
    }

    pub(crate) fn revert(&mut self) {
        let Staged {
            undo,
            compacted,
            recorded,
        } = &mut self.staged;
        *recorded = false;
        for (shape, progress) in undo.drain(..).rev() {
            let Some(bottom) = self.bottoms.get_mut(&shape) else {
                continue;
            };
            if let Some(query) = bottom.progress.compaction.query()
                && progress.compaction.query() != Some(query)
            {
                self.free_queries.push(query);
            }
            bottom.progress = progress;
        }
        for (shape, former) in compacted.drain(..) {
            match self.bottoms.get_mut(&shape) {
                Some(bottom) => {
                    let into = std::mem::replace(&mut bottom.structure, former);
                    bottom.progress.compaction = Compaction::Wanted;
                    self.retired.push(into);
                }
                None => self.retired.push(former),
            }
        }
    }

    fn advance(&mut self, ctx: &GpuFrameCtx<'_>) -> Result<()> {
        for (&shape, bottom) in &mut self.bottoms {
            let Compaction::Pending { query, value } = bottom.progress.compaction else {
                continue;
            };
            if value > ctx.completed_value {
                continue;
            }
            let mut size = [0u64; 1];
            unsafe {
                self.device.handle().get_query_pool_results(
                    self.queries,
                    query,
                    &mut size,
                    vk::QueryResultFlags::TYPE_64,
                )?
            };
            self.free_queries.push(query);
            bottom.progress.compaction = Compaction::Done;
            if size[0] == 0 || size[0] >= bottom.structure.size() {
                continue;
            }
            let into = Structure::new(
                &self.device,
                "rikka.ray.bottom",
                vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL,
                size[0],
            )?;
            self.frame
                .copies
                .push((bottom.structure.handle, into.handle));
            let former = std::mem::replace(&mut bottom.structure, into);
            self.staged.compacted.push((shape, former));
        }
        Ok(())
    }

    fn owner(&mut self, mesh: MeshId, part: u32, instance: &GpuInstance) -> Owner {
        let identity = Identity {
            mesh,
            part,
            figure: instance.figure,
            object: instance.object_id,
        };
        let count = self.occurrences.entry(identity).or_insert(0);
        let owner = Owner {
            figure: instance.figure,
            object: instance.object_id,
            occurrence: *count,
        };
        *count += 1;
        owner
    }

    fn gather(
        &mut self,
        frame: &Frame,
        built: &BuiltFrame,
        meshes: &MeshPool,
        materials: &MaterialPool,
    ) {
        for (index, (draw, instance)) in frame.draws.iter().zip(&built.instances).enumerate() {
            if instance.index_count == 0 {
                continue;
            }
            let bucket = &built.buckets[instance.bucket as usize];
            if bucket.hull || bucket.pass == MaterialPass::Overlay {
                continue;
            }
            let Some(mesh) = meshes.get(draw.mesh) else {
                continue;
            };
            let hooked = bucket.program.deforms;
            let (shape, source, hook, transform, parts, part, rebuild_after) = match hooked {
                true => {
                    let job = self.hooks.push(
                        bucket.program.shader(Variant::Hook),
                        index as u32,
                        mesh.vertex_count(),
                    );
                    let rebuild_after = match materials.reach(instance.material) {
                        Some(reach) if reach.is_finite() => REBUILD_AFTER,
                        Some(_) | None => 0,
                    };
                    let shape = Shape::Hooked {
                        mesh: draw.mesh,
                        part: draw.submesh,
                        owner: self.owner(draw.mesh, draw.submesh, instance),
                    };
                    (shape, 0, job + 1, Mat4::IDENTITY, 1, 0, rebuild_after)
                }
                false => {
                    let (shape, source) = match instance.flags & instance_flags::DEFORMED != 0 {
                        true => {
                            let base = vertex_base(instance);
                            let owner = match self.deformed.get(&(draw.mesh, base)) {
                                Some(&owner) => owner,
                                None => {
                                    let owner = self.owner(draw.mesh, WHOLE_MESH, instance);
                                    self.deformed.insert((draw.mesh, base), owner);
                                    owner
                                }
                            };
                            let shape = Shape::Deformed {
                                mesh: draw.mesh,
                                owner,
                            };
                            (shape, base)
                        }
                        false => (
                            Shape::Static(draw.mesh),
                            meshes.positions_address()
                                + u64::from(mesh.vertices.offset) * size_of::<GpuPosition>() as u64,
                        ),
                    };
                    let parts = mesh.parts.len() as u32;
                    (
                        shape,
                        source,
                        0,
                        draw.transform,
                        parts,
                        draw.submesh,
                        REBUILD_AFTER,
                    )
                }
            };
            let key = (shape, transform.to_cols_array().map(f32::to_bits));
            let at = *self.group_of.entry(key).or_insert_with(|| {
                let first_part = self.parts.len() as u32;
                self.parts
                    .extend(std::iter::repeat_n(NONE_INDEX, parts as usize));
                self.groups.push(Group {
                    shape,
                    source,
                    hook,
                    transform,
                    first_part,
                    parts,
                    drawn: 0,
                    mask: MASK_SEEN,
                    casters: 0,
                    channels_all: u32::MAX,
                    channels_any: 0,
                    cutouts: false,
                    rebuild_after,
                });
                self.groups.len() as u32 - 1
            });
            let group = &mut self.groups[at as usize];
            let slot = &mut self.parts[(group.first_part + part) as usize];
            if *slot != NONE_INDEX {
                continue;
            }
            *slot = index as u32;
            group.drawn += 1;
            group.mask |= (instance.layers & LAYER_BITS) << LAYER_SHIFT;
            if instance.flags & instance_flags::CASTS_SHADOW != 0 {
                group.casters += 1;
                group.mask |= MASK_SHADOW;
            }
            group.channels_all &= instance.light_channels;
            group.channels_any |= instance.light_channels;
            if instance.flags & (instance_flags::MASKED | instance_flags::TRANSPARENT) != 0 {
                group.cutouts = true;
            }
        }
    }

    fn schedule(&mut self, ctx: &GpuFrameCtx<'_>, group: usize, meshes: &MeshPool) -> Result<()> {
        let serial = self.serial;
        let Group {
            shape,
            source,
            rebuild_after,
            ..
        } = self.groups[group];
        match self.bottoms.get_mut(&shape) {
            Some(bottom) if bottom.used == serial => return Ok(()),
            Some(bottom) if bottom.progress.built && !shape.moves() => {
                bottom.used = serial;
                if bottom.progress.compaction == Compaction::Wanted {
                    let progress = bottom.progress;
                    self.staged.undo.push((shape, progress));
                    self.measure(ctx, shape);
                }
                return Ok(());
            }
            Some(_) | None => {}
        }
        let mesh = meshes.get(shape.mesh()).expect("gathered meshes exist");
        let parts = match shape {
            Shape::Static(_) | Shape::Deformed { .. } => &mesh.parts[..],
            Shape::Hooked { part, .. } => std::slice::from_ref(&mesh.parts[part as usize]),
        };
        let indices = meshes.index_address(mesh.index_width);
        if !self.bottoms.contains_key(&shape) {
            if !shape.moves() {
                let triangles: u64 = parts
                    .iter()
                    .map(|part| u64::from(part.submesh.index_count / 3))
                    .sum();
                if self.first_built > 0 && self.first_built + triangles > FIRST_BUILD_TRIANGLES {
                    return Ok(());
                }
                self.first_built += triangles;
            }
            let flags = shape.flags(rebuild_after);
            let probe = self.frame.push_geometries(mesh, parts, source, indices);
            let rays = self.device.ray_tracing().expect("ray tracing is enabled");
            let sizes = self.frame.sizes(rays, flags, probe.clone());
            self.frame.forget_geometries(probe);
            let structure = Structure::new(
                &self.device,
                "rikka.ray.bottom",
                vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL,
                sizes.acceleration_structure_size,
            )?;
            self.bottoms.insert(
                shape,
                Bottom {
                    structure,
                    flags,
                    build_scratch: sizes.build_scratch_size,
                    update_scratch: sizes.update_scratch_size,
                    progress: Progress {
                        built: false,
                        source: 0,
                        refits: 0,
                        compaction: match shape.moves() {
                            true => Compaction::Never,
                            false => Compaction::Wanted,
                        },
                    },
                    rebuild_after,
                    used: serial,
                },
            );
        }
        let alignment = self.alignment();
        let bottom = self.bottoms.get_mut(&shape).expect("bottom exists");
        self.staged.undo.push((shape, bottom.progress));
        bottom.used = serial;
        bottom.rebuild_after = rebuild_after;
        let refit = bottom.refits(source);
        let progress = &mut bottom.progress;
        let (mode, needed) = match refit {
            true => {
                progress.refits += 1;
                (
                    vk::BuildAccelerationStructureModeKHR::UPDATE,
                    bottom.update_scratch,
                )
            }
            false => {
                progress.refits = 0;
                (
                    vk::BuildAccelerationStructureModeKHR::BUILD,
                    bottom.build_scratch,
                )
            }
        };
        progress.built = true;
        progress.source = source;
        let geometries = self.frame.push_geometries(mesh, parts, source, indices);
        let scratch = self.frame.reserve(needed, alignment);
        self.frame.jobs.push(Job {
            flags: bottom.flags,
            mode,
            dst: bottom.structure.handle,
            geometries,
            scratch,
        });
        self.measure(ctx, shape);
        Ok(())
    }

    fn measure(&mut self, ctx: &GpuFrameCtx<'_>, shape: Shape) {
        let bottom = self.bottoms.get_mut(&shape).expect("bottom exists");
        if bottom.progress.compaction != Compaction::Wanted {
            return;
        }
        let Some(query) = self.free_queries.pop() else {
            return;
        };
        bottom.progress.compaction = Compaction::Pending {
            query,
            value: ctx.frame_value,
        };
        self.frame.measured.push((bottom.structure.handle, query));
    }

    fn alignment(&self) -> u64 {
        self.device
            .ray_tracing()
            .map_or(256, |rays| rays.scratch_alignment)
    }

    fn retire_unused(&mut self, ctx: &mut GpuFrameCtx<'_>) {
        let serial = self.serial;
        let stale: smallvec::SmallVec<[Shape; 8]> = self
            .bottoms
            .iter()
            .filter(|(shape, bottom)| shape.moves() && bottom.used + DEFORMED_KEEP < serial)
            .map(|(shape, _)| *shape)
            .collect();
        for shape in stale {
            if let Some(bottom) = self.bottoms.remove(&shape) {
                self.release(bottom);
            }
        }
        let mut bytes: u64 = self.top.as_ref().map_or(0, Structure::size)
            + self.scratch.as_ref().map_or(0, GpuBuffer::size);
        for bottom in self.bottoms.values() {
            bytes += bottom.structure.size();
        }
        if bytes <= self.budget {
            return;
        }
        let mut oldest: Vec<(u64, Shape)> = self
            .bottoms
            .iter()
            .filter(|(_, bottom)| bottom.used != serial)
            .map(|(shape, bottom)| (bottom.used, *shape))
            .collect();
        oldest.sort_unstable_by_key(|(used, _)| *used);
        for (_, shape) in oldest {
            if bytes <= self.budget {
                break;
            }
            if let Some(bottom) = self.bottoms.remove(&shape) {
                bytes -= bottom.structure.size();
                self.release(bottom);
            }
        }
        for structure in self.retired.drain(..) {
            ctx.retire.defer(ctx.frame_value, Box::new(structure));
        }
        tracing::debug!(
            target: "rikka_renderer::memory",
            bytes,
            budget = self.budget,
            "acceleration structures over budget"
        );
    }

    fn write_instances(&mut self) -> u32 {
        for group in &self.groups {
            let Some(bottom) = self.bottoms.get(&group.shape) else {
                continue;
            };
            let complete = group.drawn == group.parts && !group.cutouts;
            let uniform = group.casters == 0 || group.casters == group.drawn;
            let linked = self.linking.reaches_all(group.channels_all);
            let flags = vk::GeometryInstanceFlagsKHR::TRIANGLE_FACING_CULL_DISABLE
                | match complete && uniform && linked {
                    true => vk::GeometryInstanceFlagsKHR::FORCE_OPAQUE,
                    false => vk::GeometryInstanceFlagsKHR::FORCE_NO_OPAQUE,
                };
            let mask = match self.linking.reaches_any(group.channels_any) {
                true => group.mask,
                false => group.mask & !MASK_SHADOW,
            };
            let rows = group.transform.transpose().to_cols_array();
            let mut transform = [0.0; 12];
            transform.copy_from_slice(&rows[..12]);
            self.instances.push(GpuRayInstance {
                transform,
                custom_and_mask: (group.first_part & 0x00ff_ffff) | ((mask & 0xff) << 24),
                offset_and_flags: (group.hook & 0x00ff_ffff) | (flags.as_raw() << 24),
                reference: bottom.structure.address,
            });
        }
        self.instances.len() as u32
    }

    fn schedule_top(&mut self, count: u32) -> Result<()> {
        let flags = vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE;
        if self.top.is_none() || count > self.top_capacity {
            let capacity = count.next_power_of_two().max(MIN_INSTANCES);
            let rays = self.device.ray_tracing().expect("ray tracing is enabled");
            let geometries = [top_geometry(0)];
            let info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
                .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL)
                .flags(flags)
                .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
                .geometries(&geometries);
            let mut sizes = vk::AccelerationStructureBuildSizesInfoKHR::default();
            unsafe {
                rays.structures.get_acceleration_structure_build_sizes(
                    vk::AccelerationStructureBuildTypeKHR::DEVICE,
                    &info,
                    &[capacity],
                    &mut sizes,
                )
            };
            let top = Structure::new(
                &self.device,
                "rikka.ray.top",
                vk::AccelerationStructureTypeKHR::TOP_LEVEL,
                sizes.acceleration_structure_size,
            )?;
            if let Some(former) = self.top.replace(top) {
                self.retired.push(former);
            }
            self.top_capacity = capacity;
            self.top_scratch = sizes.build_scratch_size;
        }
        let alignment = self.alignment();
        let at = self.frame.reserve(self.top_scratch, alignment);
        self.top_job = Some((0, count, at));
        Ok(())
    }

    fn ensure_scratch(&mut self, ctx: &mut GpuFrameCtx<'_>) -> Result<()> {
        let size = self.frame.scratch + self.alignment();
        if self
            .scratch
            .as_ref()
            .is_some_and(|scratch| scratch.size() >= size)
        {
            return Ok(());
        }
        let replacement = GpuBuffer::addressable(
            self.device.clone(),
            "rikka.ray.scratch",
            size.next_power_of_two(),
            vk::BufferUsageFlags::STORAGE_BUFFER,
            Placement::Device,
        )?;
        if let Some(former) = self.scratch.replace(replacement) {
            ctx.retire.defer(ctx.frame_value, Box::new(former));
        }
        Ok(())
    }

    fn write_slot(&mut self, ctx: &mut GpuFrameCtx<'_>, count: u32) -> Result<vk::DeviceAddress> {
        let slot = &mut self.slots[ctx.frame_slot];
        if count > slot.instance_capacity {
            let capacity = count.next_power_of_two();
            let former = std::mem::replace(
                &mut slot.instances,
                instance_buffer(&self.device, capacity)?,
            );
            ctx.retire.defer(ctx.frame_value, Box::new(former));
            slot.instance_capacity = capacity;
        }
        let parts = self.parts.len() as u64;
        if parts > slot.part_capacity {
            let capacity = parts.next_power_of_two();
            let former = std::mem::replace(&mut slot.parts, part_buffer(&self.device, capacity)?);
            ctx.retire.defer(ctx.frame_value, Box::new(former));
            slot.part_capacity = capacity;
        }
        let instances: &[u8] = bytemuck::cast_slice(&self.instances);
        slot.instances
            .mapped_slice_mut()
            .expect("ray instances are host-visible")[..instances.len()]
            .copy_from_slice(instances);
        let parts: &[u8] = bytemuck::cast_slice(&self.parts);
        slot.parts
            .mapped_slice_mut()
            .expect("ray parts are host-visible")[..parts.len()]
            .copy_from_slice(parts);
        if let Some(job) = &mut self.top_job {
            job.0 = slot.instances.device_address();
        }
        Ok(slot.parts.device_address())
    }

    pub(crate) fn record(&mut self, ctx: &GpuFrameCtx<'_>) {
        let rays = self.device.ray_tracing().expect("ray tracing is enabled");
        let scratch = self.scratch.as_ref().map_or(0, |scratch| {
            scratch
                .device_address()
                .next_multiple_of(rays.scratch_alignment)
        });
        self.frame
            .record(&self.device, ctx.cmd, scratch, self.queries);
        self.staged.recorded = true;
        let (Some(top), Some((instances, count, offset))) = (&self.top, self.top_job) else {
            return;
        };
        let geometries = [top_geometry(instances)];
        let info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
            .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL)
            .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
            .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
            .dst_acceleration_structure(top.handle)
            .geometries(&geometries)
            .scratch_data(vk::DeviceOrHostAddressKHR {
                device_address: scratch + offset,
            });
        let range = [vk::AccelerationStructureBuildRangeInfoKHR::default().primitive_count(count)];
        build(rays, ctx.cmd, &info, &range);
    }
}

impl Drop for RayScene {
    fn drop(&mut self) {
        unsafe { self.device.handle().destroy_query_pool(self.queries, None) };
    }
}

fn vertex_base(instance: &GpuInstance) -> u64 {
    instance.positions + u64::from(instance.vertex_offset) * size_of::<GpuPosition>() as u64
}

fn top_geometry(instances: vk::DeviceAddress) -> vk::AccelerationStructureGeometryKHR<'static> {
    vk::AccelerationStructureGeometryKHR::default()
        .geometry_type(vk::GeometryTypeKHR::INSTANCES)
        .geometry(vk::AccelerationStructureGeometryDataKHR {
            instances: vk::AccelerationStructureGeometryInstancesDataKHR::default().data(
                vk::DeviceOrHostAddressConstKHR {
                    device_address: instances,
                },
            ),
        })
}

fn instance_buffer(device: &Arc<RenderDevice>, capacity: u32) -> rikka_gpu::Result<GpuBuffer> {
    GpuBuffer::addressable(
        device.clone(),
        "rikka.ray.instances",
        u64::from(capacity) * size_of::<GpuRayInstance>() as u64,
        vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR,
        Placement::Upload,
    )
}

fn part_buffer(device: &Arc<RenderDevice>, capacity: u64) -> rikka_gpu::Result<GpuBuffer> {
    GpuBuffer::addressable(
        device.clone(),
        "rikka.ray.parts",
        capacity * 4,
        vk::BufferUsageFlags::STORAGE_BUFFER,
        Placement::Upload,
    )
}

fn device_local_budget(device: &RenderDevice) -> u64 {
    match device.memory_budget() {
        Some(heaps) => heaps
            .iter()
            .filter(|heap| heap.device_local)
            .map(|heap| heap.budget)
            .max()
            .unwrap_or(0),
        None => {
            let properties = unsafe {
                device
                    .instance()
                    .handle()
                    .get_physical_device_memory_properties(device.physical())
            };
            properties.memory_heaps[..properties.memory_heap_count as usize]
                .iter()
                .filter(|heap| heap.flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL))
                .map(|heap| heap.size)
                .max()
                .unwrap_or(0)
        }
    }
}
