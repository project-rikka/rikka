use std::collections::BTreeMap;

use glam::Mat4;
use rikka_gpu::Compute;
use rikka_gpu::{Batch, ComputeBuffer};
use rikka_model_core::NodeId;
use rikka_physics_gpu::{MAX_SCANNED, Pass, Ring, Scan};

use crate::body::{BodyId, BodyMesh, JointReach, Region, RegionId};
use crate::error::{Error, Result};
use crate::gpu::kernels::Kernels;
use crate::gpu::settings::{Grid, NARROWEST_GRID};
use rikka_physics_gpu::REACTION_BYTES;

const VEC4: u64 = 16;
const WORD: u64 = 4;
const MATRIX: u64 = 64;
const ENTRIES_PER_TRIANGLE: u32 = 24;
const FACE_VECS: u64 = 10;
const TABLE_ENTRIES: u64 = 7;
const ADDRESS: u64 = 8;
const PLACING_BYTES: u64 = 12;
const VERTEX_GROUP: u32 = 128;
const MIN_TRIANGLE_BINS: u32 = 1 << 10;
const TRIANGLE_BINS_PER_TRIANGLE: u32 = 16;

pub(crate) struct Body {
    pub(crate) id: BodyId,
    pub(crate) mesh: BodyMesh,
    pub(crate) first_vertex: u32,
    pub(crate) first_triangle: u32,
    pub(crate) first_joint: u32,
    pub(crate) palette: Vec<Mat4>,
    pub(crate) previous: Vec<Mat4>,
    pub(crate) drapes: Vec<BodyDrape>,
    pub(crate) morph_weights: Vec<f32>,
    pub(crate) previous_morph_weights: Vec<f32>,
    pub(crate) fresh: bool,
    pub(crate) wet_ring: Ring,
    pub(crate) reactions: Vec<(NodeId, ComputeBuffer)>,
    reaction_rings: Vec<(NodeId, Ring)>,
}

#[derive(Clone, Debug)]
pub(crate) struct BodyDrape {
    pub(crate) node: NodeId,
    pub(crate) first_vertex: u32,
    pub(crate) vertices: u32,
    pub(crate) buffer: ComputeBuffer,
}

#[derive(Clone, Copy)]
struct Counts {
    vertices: u32,
    triangles: u32,
    joints: u32,
    targets: u32,
    deltas: u32,
}

pub(crate) struct BodyBuffers {
    rest: ComputeBuffer,
    joints: ComputeBuffer,
    weights: ComputeBuffer,
    pub(crate) triangles: ComputeBuffer,
    palette: ComputeBuffer,
    morph_starts: ComputeBuffer,
    morph_deltas: ComputeBuffer,
    morph_weights: ComputeBuffer,
    pub(crate) skins: [ComputeBuffer; 2],
    pub(crate) from: usize,
    pub(crate) spheres: ComputeBuffer,
    pub(crate) faces: ComputeBuffer,
    pub(crate) table: ComputeBuffer,
    pub(crate) deposits: ComputeBuffer,
    pub(crate) wet: ComputeBuffer,
    pub(crate) wet_record: ComputeBuffer,
    pub(crate) reactions: ComputeBuffer,
    cells: ComputeBuffer,
    entry_bins: ComputeBuffer,
    entry_slots: ComputeBuffer,
    entry_tris: ComputeBuffer,
    pub(crate) tri_starts: ComputeBuffer,
    pub(crate) tri_items: ComputeBuffer,
    placing: ComputeBuffer,
    pub(crate) vertices: u32,
    pub(crate) triangle_count: u32,
    targets: u32,
    entry_capacity: u32,
    tri_grid: Grid,
    table_dirty: bool,
}

impl BodyBuffers {
    pub(crate) fn skin_from(&self) -> &ComputeBuffer {
        &self.skins[self.from]
    }

    pub(crate) fn skin_to(&self) -> &ComputeBuffer {
        &self.skins[1 - self.from]
    }

    pub(crate) fn tri_offsets(&self) -> &ComputeBuffer {
        &self.cells
    }
}

pub(crate) struct Bodies {
    pub(crate) list: Vec<Body>,
    pub(crate) regions: BTreeMap<RegionId, Region>,
    pub(crate) buffers: Option<BodyBuffers>,
    pub(crate) grid: Grid,
    next_body: u32,
    next_region: u32,
    wanted_entries: u32,
    scan: Scan,
    palette: Vec<[f32; 16]>,
    weights: Vec<f32>,
}

impl Bodies {
    pub(crate) fn new(compute: &Compute, grid: Grid) -> Result<Bodies> {
        Ok(Bodies {
            list: Vec::new(),
            regions: BTreeMap::new(),
            buffers: None,
            grid,
            next_body: 0,
            next_region: 0,
            wanted_entries: 0,
            scan: Scan::new(compute)?,
            palette: Vec::new(),
            weights: Vec::new(),
        })
    }

    pub(crate) fn body(&self, id: BodyId) -> Result<&Body> {
        self.list
            .iter()
            .find(|body| body.id == id)
            .ok_or(Error::UnknownBody(id))
    }

    pub(crate) fn add(&mut self, compute: &Compute, mesh: BodyMesh) -> Result<BodyId> {
        let id = BodyId(self.next_body);
        let mut sources: Vec<Option<(u32, u32)>> = self
            .list
            .iter()
            .map(|body| Some((body.first_vertex, body.mesh.vertex_count())))
            .collect();
        sources.push(None);
        let palette = vec![Mat4::IDENTITY; mesh.palette as usize];
        let morph_weights = vec![0.0; mesh.morphs.targets() as usize];
        let wet_size = u64::from(mesh.vertex_count()) * VEC4;
        self.list.push(Body {
            id,
            mesh,
            first_vertex: 0,
            first_triangle: 0,
            first_joint: 0,
            previous: palette.clone(),
            palette,
            drapes: Vec::new(),
            previous_morph_weights: morph_weights.clone(),
            morph_weights,
            fresh: true,
            wet_ring: Ring::new(wet_size),
            reactions: Vec::new(),
            reaction_rings: Vec::new(),
        });
        if let Err(error) = self.rebuild(compute, &sources) {
            self.list.pop();
            return Err(error);
        }
        self.next_body += 1;
        Ok(id)
    }

    pub(crate) fn remove(&mut self, compute: &Compute, id: BodyId) -> Result<()> {
        let at = self
            .list
            .iter()
            .position(|body| body.id == id)
            .ok_or(Error::UnknownBody(id))?;
        let sources: Vec<Option<(u32, u32)>> = self
            .list
            .iter()
            .enumerate()
            .filter(|&(index, _)| index != at)
            .map(|(_, body)| Some((body.first_vertex, body.mesh.vertex_count())))
            .collect();
        self.regions.retain(|_, region| region.body != id);
        self.list.remove(at);
        self.rebuild(compute, &sources)
    }

    pub(crate) fn add_region(
        &mut self,
        body: BodyId,
        triangles: &[(NodeId, u32)],
    ) -> Result<RegionId> {
        let fluid_body = self.body(body)?;
        let mut local = Vec::with_capacity(triangles.len());
        for &(node, triangle) in triangles {
            let part = fluid_body
                .mesh
                .part(node)
                .ok_or(Error::MissingMesh { body, node })?;
            if triangle >= part.triangles {
                return Err(Error::TriangleOutOfRange {
                    node,
                    triangle,
                    triangles: part.triangles,
                });
            }
            local.push(part.first_triangle + triangle);
        }
        if local.is_empty() {
            return Err(Error::EmptyRegion);
        }
        let region = Region::new(body, &fluid_body.mesh, local);
        let id = RegionId(self.next_region);
        self.next_region += 1;
        self.regions.insert(id, region);
        Ok(id)
    }

    pub(crate) fn remove_region(&mut self, region: RegionId) -> bool {
        self.regions.remove(&region).is_some()
    }

    pub(crate) fn region(&self, id: RegionId) -> Option<(&Region, u32)> {
        let region = self.regions.get(&id)?;
        let body = self.body(region.body).ok()?;
        Some((region, body.first_triangle))
    }

    pub(crate) fn travel(&self) -> f32 {
        self.list
            .iter()
            .map(|body| {
                JointReach::travel(&body.mesh.reach, &body.previous, &body.palette)
                    + body
                        .mesh
                        .morphs
                        .travel(&body.previous_morph_weights, &body.morph_weights)
            })
            .fold(0.0, f32::max)
    }

    pub(crate) fn note_entries(&mut self, entries: u32) {
        self.wanted_entries = entries;
    }

    pub(crate) fn active_grid(&self) -> Option<Grid> {
        self.buffers.as_ref().map(|buffers| buffers.tri_grid)
    }

    pub(crate) fn has_drapes(&self) -> bool {
        self.list.iter().any(|body| !body.drapes.is_empty())
    }

    fn rebuild(&mut self, compute: &Compute, sources: &[Option<(u32, u32)>]) -> Result<()> {
        let mut vertices = 0u32;
        let mut triangles = 0u32;
        let mut joints = 0u32;
        let mut targets = 0u32;
        let mut deltas = 0u32;
        for body in &mut self.list {
            body.first_vertex = vertices;
            body.first_triangle = triangles;
            body.first_joint = joints;
            vertices = vertices.checked_add(body.mesh.vertex_count()).ok_or(
                Error::TooManyBodyVertices {
                    count: u64::from(vertices) + u64::from(body.mesh.vertex_count()),
                    limit: u32::MAX,
                },
            )?;
            triangles += body.mesh.triangle_count();
            joints += body.mesh.palette;
            targets += body.mesh.morphs.targets();
            deltas += body.mesh.morphs.deltas.len() as u32;
        }
        if self.list.is_empty() {
            self.buffers = None;
            return Ok(());
        }
        if triangles >= MAX_SCANNED {
            return Err(Error::TooManyBodyTriangles {
                count: triangles,
                limit: MAX_SCANNED,
            });
        }
        let max_vertices = compute.widest_launch().saturating_mul(VERTEX_GROUP);
        if vertices > max_vertices {
            return Err(Error::TooManyBodyVertices {
                count: u64::from(vertices),
                limit: max_vertices,
            });
        }
        let mut buffers = self.allocate(
            compute,
            Counts {
                vertices,
                triangles,
                joints,
                targets,
                deltas,
            },
        )?;
        let old = self.buffers.take();
        let mut batch = compute.batch();
        let mut rest = Vec::with_capacity(vertices as usize);
        let mut joint_words = Vec::with_capacity(vertices as usize);
        let mut weights = Vec::with_capacity(vertices as usize);
        let mut corners = Vec::with_capacity(triangles as usize);
        let mut morph_starts = Vec::with_capacity(vertices as usize + 1);
        let mut morph_deltas = Vec::with_capacity(deltas as usize);
        let mut first_target = 0u32;
        for (index, body) in self.list.iter().enumerate() {
            let morphs = &body.mesh.morphs;
            let base = morph_deltas.len() as u32;
            morph_starts.extend(
                morphs.starts[..morphs.starts.len() - 1]
                    .iter()
                    .map(|start| start + base),
            );
            morph_deltas.extend(
                morphs
                    .deltas
                    .iter()
                    .map(|&[x, y, z, target]| [x, y, z, target + first_target as f32]),
            );
            first_target += morphs.targets();
            rest.extend_from_slice(&body.mesh.rest);
            joint_words.extend(
                body.mesh
                    .joints
                    .iter()
                    .map(|joint| joint.map(|j| j + body.first_joint)),
            );
            weights.extend_from_slice(&body.mesh.weights);
            corners.extend(body.mesh.triangles.iter().map(|tri| {
                [
                    tri[0] + body.first_vertex,
                    tri[1] + body.first_vertex,
                    tri[2] + body.first_vertex,
                    index as u32,
                ]
            }));
        }
        batch.write(&buffers.rest, 0, bytemuck::cast_slice(&rest));
        batch.write(&buffers.joints, 0, bytemuck::cast_slice(&joint_words));
        batch.write(&buffers.weights, 0, bytemuck::cast_slice(&weights));
        batch.write(&buffers.triangles, 0, bytemuck::cast_slice(&corners));
        morph_starts.push(morph_deltas.len() as u32);
        batch.write(
            &buffers.morph_starts,
            0,
            bytemuck::cast_slice(&morph_starts),
        );
        if !morph_deltas.is_empty() {
            batch.write(
                &buffers.morph_deltas,
                0,
                bytemuck::cast_slice(&morph_deltas),
            );
        }
        batch.fill(&buffers.deposits, 0);
        batch.fill(&buffers.wet, 0);
        batch.fill(&buffers.wet_record, 0);
        batch.fill(&buffers.reactions, 0);
        if let Some(old) = &old {
            buffers.from = old.from;
            for (body, source) in self.list.iter().zip(sources) {
                let Some((first, count)) = *source else {
                    continue;
                };
                let bytes = u64::from(count) * VEC4;
                let from = u64::from(first) * VEC4;
                let to = u64::from(body.first_vertex) * VEC4;
                batch.copy_buffer(&old.wet, from, &buffers.wet, to, bytes);
                batch.copy_buffer(&old.skins[0], from, &buffers.skins[0], to, bytes);
                batch.copy_buffer(&old.skins[1], from, &buffers.skins[1], to, bytes);
            }
        }
        batch.submit()?;
        self.buffers = Some(buffers);
        Ok(())
    }

    fn allocate(&self, compute: &Compute, counts: Counts) -> Result<BodyBuffers> {
        let Counts {
            vertices,
            triangles,
            joints,
            targets,
            deltas,
        } = counts;
        let wide = u64::from(vertices);
        let entry_capacity = self
            .wanted_entries
            .max(triangles.saturating_mul(ENTRIES_PER_TRIANGLE))
            .clamp(1024, MAX_SCANNED);
        let entries = u64::from(entry_capacity) * WORD;
        let tri_bins = triangles
            .saturating_mul(TRIANGLE_BINS_PER_TRIANGLE)
            .clamp(MIN_TRIANGLE_BINS, self.grid.bins());
        let tri_grid = Grid::sized(tri_bins, NARROWEST_GRID);
        Ok(BodyBuffers {
            rest: compute.buffer(wide * VEC4)?,
            joints: compute.buffer(wide * VEC4)?,
            weights: compute.buffer(wide * 8)?,
            triangles: compute.buffer(u64::from(triangles) * VEC4)?,
            palette: compute.buffer(u64::from(joints) * MATRIX)?,
            morph_starts: compute.buffer((wide + 1) * WORD)?,
            morph_deltas: compute.buffer(u64::from(deltas.max(1)) * VEC4)?,
            morph_weights: compute.buffer(u64::from(targets.max(1)) * WORD)?,
            skins: [compute.buffer(wide * VEC4)?, compute.buffer(wide * VEC4)?],
            from: 0,
            spheres: compute.buffer(u64::from(triangles) * 2 * VEC4)?,
            faces: compute.buffer(u64::from(triangles) * FACE_VECS * VEC4)?,
            table: compute.buffer(TABLE_ENTRIES * ADDRESS)?,
            deposits: compute.buffer(wide * VEC4)?,
            wet: compute.buffer(wide * VEC4)?,
            wet_record: compute.buffer(wide * WORD)?,
            reactions: compute.buffer(wide * REACTION_BYTES)?,
            cells: compute.buffer((u64::from(triangles) + 1) * WORD)?,
            entry_bins: compute.buffer(entries)?,
            entry_slots: compute.buffer(entries)?,
            entry_tris: compute.buffer(entries)?,
            tri_starts: compute.buffer((u64::from(tri_grid.bins()) + 2) * WORD)?,
            tri_items: compute.buffer(entries)?,
            placing: compute.buffer(PLACING_BYTES)?,
            vertices,
            triangle_count: triangles,
            targets,
            entry_capacity,
            tri_grid,
            table_dirty: true,
        })
    }

    pub(crate) fn grow_entries(&mut self, compute: &Compute) -> Result<()> {
        let capacity = self
            .buffers
            .as_ref()
            .map_or(0, |buffers| buffers.entry_capacity);
        if self.wanted_entries <= capacity || capacity >= MAX_SCANNED {
            return Ok(());
        }
        let capacity = self
            .wanted_entries
            .saturating_add(self.wanted_entries / 4)
            .min(MAX_SCANNED);
        self.set_entries(compute, self.wanted_entries, capacity)
    }

    pub(crate) fn pose(&mut self, batch: &mut Batch, kernels: &Kernels, settings: &ComputeBuffer) {
        self.skin(batch, kernels, true);
        self.index(batch, kernels, settings);
        if let Some(buffers) = &mut self.buffers
            && buffers.table_dirty
        {
            let addresses = [
                &buffers.faces,
                &buffers.spheres,
                &buffers.triangles,
                &buffers.tri_starts,
                &buffers.tri_items,
                &buffers.deposits,
                &buffers.reactions,
            ]
            .map(ComputeBuffer::address);
            batch.write(&buffers.table, 0, bytemuck::cast_slice(&addresses));
            buffers.table_dirty = false;
        }
    }

    pub(crate) fn skin(&mut self, batch: &mut Batch, kernels: &Kernels, swap: bool) {
        let Some(buffers) = &mut self.buffers else {
            return;
        };
        self.palette.clear();
        for body in &self.list {
            self.palette
                .extend(body.palette.iter().map(|matrix| matrix.to_cols_array()));
        }
        batch.write(&buffers.palette, 0, bytemuck::cast_slice(&self.palette));
        if buffers.targets > 0 {
            self.weights.clear();
            for body in &self.list {
                self.weights.extend_from_slice(&body.morph_weights);
            }
            batch.write(
                &buffers.morph_weights,
                0,
                bytemuck::cast_slice(&self.weights),
            );
        }
        if swap {
            buffers.from = 1 - buffers.from;
        }
        let to = &buffers.skins[1 - buffers.from];
        Pass::direct(&kernels.skin, buffers.vertices, VERTEX_GROUP)
            .read(&buffers.rest)
            .read(&buffers.joints)
            .read(&buffers.weights)
            .read(&buffers.palette)
            .read(&buffers.morph_starts)
            .read(&buffers.morph_deltas)
            .read(&buffers.morph_weights)
            .write(to)
            .uint(buffers.vertices)
            .uint(buffers.targets)
            .run(batch);
        for body in &self.list {
            for drape in &body.drapes {
                Pass::direct(&kernels.drape, drape.vertices, VERTEX_GROUP)
                    .read(&drape.buffer)
                    .write(to)
                    .uint(body.first_vertex + drape.first_vertex)
                    .uint(drape.vertices)
                    .run(batch);
            }
        }
        for body in &mut self.list {
            if body.fresh || !swap {
                let at = u64::from(body.first_vertex) * VEC4;
                let bytes = u64::from(body.mesh.vertex_count()) * VEC4;
                batch.copy_buffer(to, at, &buffers.skins[buffers.from], at, bytes);
                body.fresh = false;
            }
        }
    }

    fn index(&self, batch: &mut Batch, kernels: &Kernels, settings: &ComputeBuffer) {
        let Some(buffers) = &self.buffers else {
            return;
        };
        let triangles = buffers.triangle_count;
        Pass::direct(&kernels.tri_cells, triangles + 1, 128)
            .read(settings)
            .read(buffers.skin_from())
            .read(buffers.skin_to())
            .read(&buffers.triangles)
            .write(&buffers.cells)
            .uint(triangles)
            .run(batch);
        self.scan.run(batch, &buffers.cells, triangles + 1);
        let bins = buffers.tri_grid.bins();
        batch.fill_range(&buffers.tri_starts, 0, (u64::from(bins) + 2) * WORD, 0);
        Pass::direct(&kernels.tri_bins, triangles, 128)
            .read(settings)
            .read(buffers.skin_from())
            .read(buffers.skin_to())
            .read(&buffers.triangles)
            .read(&buffers.cells)
            .write(&buffers.entry_bins)
            .write(&buffers.entry_slots)
            .write(&buffers.entry_tris)
            .write(&buffers.tri_starts)
            .write(&buffers.placing)
            .uint(triangles)
            .uint(buffers.entry_capacity)
            .run(batch);
        self.scan.run(batch, &buffers.tri_starts, bins + 2);
        Pass::indirect(&kernels.tri_place, &buffers.placing, 0)
            .read(&buffers.cells)
            .read(&buffers.entry_bins)
            .read(&buffers.entry_slots)
            .read(&buffers.entry_tris)
            .read(&buffers.tri_starts)
            .write(&buffers.tri_items)
            .uint(triangles)
            .uint(buffers.entry_capacity)
            .run(batch);
    }

    pub(crate) fn entries(&self) -> (u32, u32) {
        (
            self.wanted_entries,
            self.buffers
                .as_ref()
                .map_or(0, |buffers| buffers.entry_capacity),
        )
    }

    pub(crate) fn set_entries(
        &mut self,
        compute: &Compute,
        entries: u32,
        capacity: u32,
    ) -> Result<()> {
        self.wanted_entries = entries;
        let Some(buffers) = &mut self.buffers else {
            return Ok(());
        };
        if capacity == 0 || capacity == buffers.entry_capacity {
            return Ok(());
        }
        let entries = u64::from(capacity) * WORD;
        buffers.entry_bins = compute.buffer(entries)?;
        buffers.entry_slots = compute.buffer(entries)?;
        buffers.entry_tris = compute.buffer(entries)?;
        buffers.tri_items = compute.buffer(entries)?;
        buffers.entry_capacity = capacity;
        buffers.table_dirty = true;
        Ok(())
    }

    pub(crate) fn dry(&self, batch: &mut Batch, kernels: &Kernels, keep: f32) {
        let Some(buffers) = &self.buffers else {
            return;
        };
        Pass::direct(&kernels.wet_update, buffers.vertices, VERTEX_GROUP)
            .write(&buffers.deposits)
            .write(&buffers.wet)
            .write(&buffers.wet_record)
            .uint(buffers.vertices)
            .float(keep)
            .run(batch);
    }

    pub(crate) fn wet_out(
        &self,
        batch: &mut Batch,
        kernels: &Kernels,
        body: &Body,
        into: &ComputeBuffer,
    ) {
        let Some(buffers) = &self.buffers else {
            return;
        };
        Pass::direct(&kernels.wet_out, body.mesh.vertex_count(), VERTEX_GROUP)
            .read(&buffers.wet_record)
            .write(into)
            .uint(body.first_vertex)
            .uint(body.mesh.vertex_count())
            .run(batch);
    }

    pub(crate) fn clear_reactions(&self, batch: &mut Batch) {
        if let Some(buffers) = &self.buffers {
            batch.fill(&buffers.reactions, 0);
        }
    }

    pub(crate) fn react_out(&mut self, batch: &mut Batch, compute: &Compute) -> Result<()> {
        let Bodies { list, buffers, .. } = self;
        let Some(buffers) = buffers else {
            return Ok(());
        };
        for body in list {
            body.reactions.clear();
            for at in 0..body.drapes.len() {
                let BodyDrape {
                    node,
                    first_vertex,
                    vertices,
                    ..
                } = body.drapes[at];
                let out = body.reaction_ring(node, vertices).acquire(compute)?;
                batch.copy_buffer(
                    &buffers.reactions,
                    u64::from(body.first_vertex + first_vertex) * REACTION_BYTES,
                    &out,
                    0,
                    u64::from(vertices) * REACTION_BYTES,
                );
                body.reactions.push((node, out));
            }
        }
        Ok(())
    }
}

impl Body {
    pub(crate) fn rest_pose(&mut self) {
        self.palette.fill(Mat4::IDENTITY);
        self.previous.fill(Mat4::IDENTITY);
        self.morph_weights.fill(0.0);
        self.previous_morph_weights.fill(0.0);
        self.drapes.clear();
        self.reactions.clear();
        self.fresh = true;
    }

    pub(crate) fn reaction_ring(&mut self, node: NodeId, vertices: u32) -> &mut Ring {
        let at = match self
            .reaction_rings
            .iter()
            .position(|(ring_node, _)| *ring_node == node)
        {
            Some(at) => at,
            None => {
                self.reaction_rings
                    .push((node, Ring::new(u64::from(vertices) * REACTION_BYTES)));
                self.reaction_rings.len() - 1
            }
        };
        &mut self.reaction_rings[at].1
    }
}
