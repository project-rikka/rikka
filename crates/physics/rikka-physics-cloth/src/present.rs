use rikka_gpu::{Batch, ComputeBuffer};
use rikka_physics_gpu::Pass;

use crate::cloth::Cloth;
use crate::colliders::place_palettes;
use crate::error::Result;
use crate::figure::{Figure, FigureId};
use crate::inputs::Inputs;
use crate::launch::WIDTH;
use crate::step::StepPlan;

const SOURCE_PRESENT: u32 = 1;

impl Cloth {
    pub(crate) fn skin(&self, batch: &mut Batch, figures: &[&Figure], boundaries: u32) {
        for figure in figures {
            let body = figure.body_vertices();
            if body == 0 {
                continue;
            }
            let sections = figure.sections;
            Pass::direct(&self.kernels.skin_body, boundaries * body, WIDTH)
                .read_at(&figure.statics, sections.body_rest)
                .read_at(&figure.statics, sections.body_joints)
                .read_at(&figure.statics, sections.body_weights)
                .read_at(&figure.frame, figure.frame_layout.palettes)
                .write_at(&figure.posed, figure.posed_layout.body)
                .uint(body)
                .uint(boundaries)
                .uint(figure.topology.slots.len() as u32)
                .run(batch);
        }
        for figure in figures {
            let sections = figure.sections;
            Pass::direct(&self.kernels.targets, boundaries * figure.count(), WIDTH)
                .read_at(&figure.statics, sections.rest)
                .read_at(&figure.statics, sections.anchors)
                .read_at(&figure.frame, figure.frame_layout.palettes)
                .read_at(&figure.posed, figure.posed_layout.body)
                .read_at(&figure.statics, sections.body_triangles)
                .write_at(&figure.scratch, figure.scratch_layout.targets)
                .uint(figure.count())
                .uint(boundaries)
                .uint(figure.topology.slots.len() as u32)
                .uint(figure.body_vertices())
                .run(batch);
        }
    }

    pub(crate) fn present(
        &mut self,
        batch: &mut Batch,
        shown: &[FigureId],
        boundary: u32,
        upload_colliders: bool,
    ) -> Result<()> {
        let mut draped: Vec<(FigureId, Vec<ComputeBuffer>)> = Vec::new();
        for id in shown {
            let Some(figure) = self.figures.get_mut(id) else {
                continue;
            };
            let binds = figure.topology.binds.len();
            let Some(outputs) = figure.outputs.as_mut() else {
                continue;
            };
            let buffers: Vec<ComputeBuffer> = outputs.acquire(&self.compute)?;
            if binds == 0 || buffers.is_empty() {
                continue;
            }
            let addresses: Vec<u64> = buffers.iter().map(ComputeBuffer::address).collect();
            batch.write(
                &figure.frame,
                figure.frame_layout.outputs,
                bytemuck::cast_slice(&addresses),
            );
            for buffer in &buffers {
                batch.fill(buffer, 0);
            }
            draped.push((*id, buffers));
        }
        let up = -self.settings.gravity.normalize_or_zero();
        let figures: Vec<&Figure> = shown.iter().filter_map(|id| self.figures.get(id)).collect();
        for figure in &figures {
            let sections = figure.sections;
            let count = figure.count();
            Pass::direct(&self.kernels.normals, count, WIDTH)
                .read(&figure.state)
                .read_at(&figure.statics, sections.triangles)
                .read_at(&figure.statics, sections.incident_starts)
                .read_at(&figure.statics, sections.incident)
                .write(&figure.normals)
                .uint(count)
                .run(batch);
        }
        for (id, _) in &draped {
            let figure = &self.figures[id];
            if !figure.topology.layers.iter().any(|&layer| layer > 0) {
                continue;
            }
            let sections = figure.sections;
            let binds = figure.topology.binds.len() as u32;
            Pass::direct(&self.kernels.lift, binds, WIDTH)
                .read(&figure.state)
                .read(&figure.normals)
                .read_at(&figure.statics, sections.triangles)
                .read_at(&figure.statics, sections.binds)
                .read_at(&figure.scratch, figure.scratch_layout.nearest)
                .read_at(&figure.statics, sections.rest)
                .read_at(&figure.statics, sections.reliefs)
                .read_at(&figure.statics, sections.incident_starts)
                .read_at(&figure.statics, sections.incident)
                .write(&figure.normals)
                .uint(binds)
                .float(up.x)
                .float(up.y)
                .float(up.z)
                .run(batch);
        }
        for (id, buffers) in &draped {
            let figure = &self.figures[id];
            if upload_colliders && !figure.colliders.is_empty() && !figure.shown.is_empty() {
                let mut colliders = Vec::with_capacity(figure.colliders.len());
                let mut planes = Vec::with_capacity(figure.planes);
                place_palettes(
                    &figure.colliders,
                    &figure.shown,
                    &mut colliders,
                    &mut planes,
                );
                batch.write(
                    &figure.frame,
                    figure.colliders_offset(boundary),
                    bytemuck::cast_slice(&colliders),
                );
                if !planes.is_empty() {
                    batch.write(
                        &figure.frame,
                        figure.planes_offset(boundary),
                        bytemuck::cast_slice(&planes),
                    );
                }
            }
            let sections = figure.sections;
            let binds = figure.topology.binds.len() as u32;
            let collider_count = if figure.shown.is_empty() {
                0
            } else {
                figure.colliders.len() as u32
            };
            let mut pass = Pass::direct(&self.kernels.drape, binds, WIDTH)
                .read(&figure.state)
                .read(&figure.normals)
                .read_at(&figure.statics, sections.triangles)
                .read_at(&figure.statics, sections.binds)
                .read_at(&figure.statics, sections.strand_nodes)
                .read_at(&figure.frame, figure.frame_layout.outputs)
                .read(&figure.hands)
                .read_at(&figure.scratch, figure.scratch_layout.nearest)
                .read_at(&figure.posed, figure.body_offset(boundary))
                .read_at(&figure.statics, sections.body_triangles)
                .read_at(&figure.frame, figure.colliders_offset(boundary))
                .read_at(&figure.frame, figure.planes_offset(boundary))
                .uint(binds)
                .uint(collider_count)
                .float(up.x)
                .float(up.y)
                .float(up.z);
            for buffer in buffers {
                pass = pass.writes_through(buffer);
            }
            pass.run(batch);
        }
        Ok(())
    }

    pub(crate) fn absorb(&self, batch: &mut Batch, inputs: &Inputs, last: &[StepPlan]) {
        let mut absorbing = Vec::new();
        for step_plan in last {
            let figure = &self.figures[&step_plan.figure];
            let mut sources = vec![[0u32; 4]; figure.topology.nodes.len()];
            let mut reactions = Vec::new();
            for (id, node, buffer) in &inputs.reactions {
                if *id != step_plan.figure {
                    continue;
                }
                if let Ok(slot) = figure.topology.nodes.binary_search(node) {
                    let address = buffer.address();
                    sources[slot] = [address as u32, (address >> 32) as u32, SOURCE_PRESENT, 0];
                    reactions.push(buffer);
                }
            }
            if reactions.is_empty() {
                continue;
            }
            batch.write(
                &figure.frame,
                figure.frame_layout.sources,
                bytemuck::cast_slice(&sources),
            );
            absorbing.push((figure, reactions));
        }
        for (figure, reactions) in absorbing {
            let sections = figure.sections;
            let mut pass = Pass::direct(&self.kernels.absorb, figure.count(), WIDTH)
                .write_at(&figure.state, figure.velocities())
                .read_at(&figure.statics, sections.traits)
                .read_at(&figure.statics, sections.absorb_starts)
                .read_at(&figure.statics, sections.absorb)
                .read_at(&figure.frame, figure.frame_layout.sources)
                .uint(figure.count());
            for buffer in reactions {
                pass = pass.reads_through(buffer);
            }
            pass.run(batch);
        }
    }
}
