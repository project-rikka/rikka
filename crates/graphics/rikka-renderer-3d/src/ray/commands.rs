use std::ops::Range;

use ash::vk;

use crate::mesh::{IndexWidth, MeshEntry};
use crate::reach::Part;
use crate::vertex::GpuPosition;
use rikka_gpu::core::barrier::{MemoryAccess, record_memory_barrier};
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::ray_support::RayTracing;

pub(super) struct Job {
    pub flags: vk::BuildAccelerationStructureFlagsKHR,
    pub mode: vk::BuildAccelerationStructureModeKHR,
    pub dst: vk::AccelerationStructureKHR,
    pub geometries: Range<usize>,
    pub scratch: u64,
}

#[derive(Default)]
pub(super) struct Batch {
    pub geometries: Vec<vk::AccelerationStructureGeometryKHR<'static>>,
    pub ranges: Vec<vk::AccelerationStructureBuildRangeInfoKHR>,
    pub jobs: Vec<Job>,
    pub copies: Vec<(vk::AccelerationStructureKHR, vk::AccelerationStructureKHR)>,
    pub measured: Vec<(vk::AccelerationStructureKHR, u32)>,
    pub scratch: u64,
}

impl Batch {
    pub(super) fn clear(&mut self) {
        self.geometries.clear();
        self.ranges.clear();
        self.jobs.clear();
        self.copies.clear();
        self.measured.clear();
        self.scratch = 0;
    }

    pub(super) fn push_geometries(
        &mut self,
        mesh: &MeshEntry,
        parts: &[Part],
        vertices: vk::DeviceAddress,
        indices: vk::DeviceAddress,
    ) -> Range<usize> {
        let start = self.geometries.len();
        let width = match mesh.index_width {
            IndexWidth::U16 => 2,
            IndexWidth::U32 => 4,
        };
        for part in parts {
            let first = u64::from(mesh.indices.offset) + u64::from(part.submesh.first_index);
            let triangles = vk::AccelerationStructureGeometryTrianglesDataKHR::default()
                .vertex_format(vk::Format::R32G32B32_SFLOAT)
                .vertex_data(vk::DeviceOrHostAddressConstKHR {
                    device_address: vertices,
                })
                .vertex_stride(size_of::<GpuPosition>() as u64)
                .max_vertex(mesh.vertex_count().saturating_sub(1))
                .index_type(mesh.index_width.index_type())
                .index_data(vk::DeviceOrHostAddressConstKHR {
                    device_address: indices + first * width,
                });
            self.geometries.push(
                vk::AccelerationStructureGeometryKHR::default()
                    .geometry_type(vk::GeometryTypeKHR::TRIANGLES)
                    .geometry(vk::AccelerationStructureGeometryDataKHR { triangles }),
            );
            self.ranges.push(
                vk::AccelerationStructureBuildRangeInfoKHR::default()
                    .primitive_count(part.submesh.index_count / 3),
            );
        }
        start..self.geometries.len()
    }

    pub(super) fn forget_geometries(&mut self, geometries: Range<usize>) {
        self.geometries.truncate(geometries.start);
        self.ranges.truncate(geometries.start);
    }

    pub(super) fn sizes(
        &self,
        rays: &RayTracing,
        flags: vk::BuildAccelerationStructureFlagsKHR,
        geometries: Range<usize>,
    ) -> vk::AccelerationStructureBuildSizesInfoKHR<'static> {
        let info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
            .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
            .flags(flags)
            .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
            .geometries(&self.geometries[geometries.clone()]);
        let counts: smallvec::SmallVec<[u32; 32]> = self.ranges[geometries]
            .iter()
            .map(|range| range.primitive_count)
            .collect();
        let mut sizes = vk::AccelerationStructureBuildSizesInfoKHR::default();
        unsafe {
            rays.structures.get_acceleration_structure_build_sizes(
                vk::AccelerationStructureBuildTypeKHR::DEVICE,
                &info,
                &counts,
                &mut sizes,
            )
        };
        sizes
    }

    pub(super) fn reserve(&mut self, needed: u64, alignment: u64) -> u64 {
        let at = self.scratch.next_multiple_of(alignment);
        self.scratch = at + needed;
        at
    }

    pub(super) fn record(
        &self,
        device: &RenderDevice,
        cmd: vk::CommandBuffer,
        scratch: vk::DeviceAddress,
        queries: vk::QueryPool,
    ) {
        let rays = device.ray_tracing().expect("ray tracing is enabled");
        let structures = MemoryAccess::new(
            vk::PipelineStageFlags2::ACCELERATION_STRUCTURE_BUILD_KHR,
            vk::AccessFlags2::ACCELERATION_STRUCTURE_READ_KHR
                | vk::AccessFlags2::ACCELERATION_STRUCTURE_WRITE_KHR,
        );
        for &(src, dst) in &self.copies {
            let info = vk::CopyAccelerationStructureInfoKHR::default()
                .src(src)
                .dst(dst)
                .mode(vk::CopyAccelerationStructureModeKHR::COMPACT);
            unsafe { rays.structures.cmd_copy_acceleration_structure(cmd, &info) };
        }
        for job in &self.jobs {
            let info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
                .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
                .flags(job.flags)
                .mode(job.mode)
                .src_acceleration_structure(match job.mode {
                    vk::BuildAccelerationStructureModeKHR::UPDATE => job.dst,
                    _ => vk::AccelerationStructureKHR::null(),
                })
                .dst_acceleration_structure(job.dst)
                .geometries(&self.geometries[job.geometries.clone()])
                .scratch_data(vk::DeviceOrHostAddressKHR {
                    device_address: scratch + job.scratch,
                });
            build(rays, cmd, &info, &self.ranges[job.geometries.clone()]);
        }
        if !self.jobs.is_empty() || !self.copies.is_empty() || !self.measured.is_empty() {
            record_memory_barrier(device, cmd, structures, structures);
        }
        for &(structure, query) in &self.measured {
            unsafe {
                device.handle().cmd_reset_query_pool(cmd, queries, query, 1);
                rays.structures
                    .cmd_write_acceleration_structures_properties(
                        cmd,
                        &[structure],
                        vk::QueryType::ACCELERATION_STRUCTURE_COMPACTED_SIZE_KHR,
                        queries,
                        query,
                    );
            }
        }
    }
}

pub(super) fn build(
    rays: &RayTracing,
    cmd: vk::CommandBuffer,
    info: &vk::AccelerationStructureBuildGeometryInfoKHR<'_>,
    ranges: &[vk::AccelerationStructureBuildRangeInfoKHR],
) {
    let ranges = ranges.as_ptr();
    unsafe {
        (rays.structures.fp().cmd_build_acceleration_structures_khr)(cmd, 1, info, &ranges);
    }
}
