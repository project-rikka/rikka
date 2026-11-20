use glam::Vec3;
use rikka_gpu::Batch;
use rikka_physics_gpu::Pass;

use crate::cloth::Cloth;
use crate::figure::{Figure, FigureId, FrameLayout, VEC4};
use crate::gpu::Variant;
use crate::launch::{NARROW_WIDTH, PROBE_LANES, WIDTH};
use crate::layout::{FigureRecord, JobRecord, NONE};
use crate::step::jobs::{Context, table_offset};
use crate::step::{JOB_BYTES, StepPlan, TABLE_BYTES};

pub(crate) struct Environment {
    pub(crate) time: f32,
    pub(crate) wind: Vec3,
    pub(crate) turbulence: f32,
}

struct Solve {
    key: (u32, u8, Variant),
    figure: FigureId,
    job: JobRecord,
}

struct Epoch {
    table: u64,
    probes: u32,
    first: u32,
    solves: Vec<Solve>,
}

impl Cloth {
    pub(crate) fn record(
        &self,
        batch: &mut Batch,
        round: &[StepPlan],
        environment: &Environment,
        measured: bool,
        cursors: &mut (u64, u64),
    ) {
        if round.is_empty() {
            return;
        }
        if let Some(tracer) = &self.tracer {
            tracer.begin_round();
        }
        let epochs = self.settings.effort.epochs;
        let figures: Vec<&Figure> = round
            .iter()
            .map(|step_plan| &self.figures[&step_plan.figure])
            .collect();
        for (step_plan, figure) in round.iter().zip(&figures) {
            batch.write(&figure.frame, 0, &framed(figure.frame_layout, step_plan));
        }
        let table_base = cursors.1;
        let table = self.tables.address() + table_base;
        let records: Vec<FigureRecord> = (0..epochs)
            .flat_map(|epoch| {
                figures
                    .iter()
                    .map(move |figure| self.table_record(figure, epoch))
            })
            .collect();
        batch.write(&self.tables, table_base, bytemuck::cast_slice(&records));
        cursors.1 += records.len() as u64 * TABLE_BYTES;
        let mut jobs: Vec<JobRecord> = Vec::new();
        let mut plans = Vec::new();
        for epoch in 0..epochs {
            let table = table_offset(table, figures.len(), epoch);
            let probes = jobs.len() as u32;
            for (index, (step_plan, figure)) in round.iter().zip(&figures).enumerate() {
                let context = Context {
                    environment,
                    table,
                    index: index as u32,
                    figures: figures.len() as u32,
                    epoch,
                    stats: NONE,
                };
                jobs.push(self.probe_job(figure, step_plan, &context));
            }
            let mut solves = Vec::new();
            for (index, (step_plan, figure)) in round.iter().zip(&figures).enumerate() {
                let context = Context {
                    environment,
                    table,
                    index: index as u32,
                    figures: figures.len() as u32,
                    epoch,
                    stats: if measured { index as u32 } else { NONE },
                };
                for group in &figure.topology.groups {
                    solves.push(Solve {
                        key: (
                            epoch,
                            group.layer,
                            self.solvers.variant(group.particles.count),
                        ),
                        figure: step_plan.figure,
                        job: self.solve_job(figure, step_plan, group, &context),
                    });
                }
            }
            solves.sort_by_key(|solve| solve.key);
            let first = jobs.len() as u32;
            jobs.extend(solves.iter().map(|solve| solve.job));
            plans.push(Epoch {
                table,
                probes,
                first,
                solves,
            });
        }
        let base = cursors.0;
        batch.write(&self.jobs, base, bytemuck::cast_slice(&jobs));
        cursors.0 += jobs.len() as u64 * JOB_BYTES;
        self.snapshot(batch, &figures, 0);
        self.skin(batch, &figures, epochs + 1);
        self.tap(batch, "skin", None, &figures);
        for (epoch, plan) in plans.into_iter().enumerate() {
            let epoch = epoch as u32;
            if epoch > 0 {
                self.snapshot(batch, &figures, epoch);
            }
            self.collide(batch, &figures, &plan, epoch, base);
            let measuring = measured && epoch + 1 == epochs;
            self.solve(batch, &figures, &plan, base, measuring);
            self.tap(batch, "solve", Some(epoch), &figures);
        }
    }

    fn snapshot(&self, batch: &mut Batch, figures: &[&Figure], epoch: u32) {
        for figure in figures {
            batch.copy_buffer(
                &figure.state,
                0,
                &figure.posed,
                figure.posed_layout.epoch_start,
                2 * u64::from(figure.count()) * VEC4,
            );
        }
        self.grid.clear(batch);
        self.tap(batch, "epoch", Some(epoch), figures);
    }

    fn collide(&self, batch: &mut Batch, figures: &[&Figure], plan: &Epoch, epoch: u32, base: u64) {
        let table_at = plan.table - self.tables.address();
        let obstacles: u32 = figures
            .iter()
            .map(|figure| figure.body_triangles() + figure.topology.triangles.len() as u32)
            .sum();
        for placing in [0u32, 1] {
            if placing == 1 {
                self.grid.scan(batch, &self.scan);
            }
            let mut pass = Pass::direct(&self.kernels.obstacles, obstacles, WIDTH)
                .read_at(&self.tables, table_at)
                .write(&self.grid.cells)
                .write(&self.grid.items)
                .write(&self.grid.spheres)
                .write(&self.grid.large)
                .uint(figures.len() as u32)
                .uint(placing)
                .float(self.settings.epoch_span());
            for figure in figures {
                pass = pass
                    .reads_through(&figure.posed)
                    .reads_through(&figure.statics);
            }
            pass.run(batch);
        }
        self.tap_grid(batch, epoch);
        for (index, figure) in figures.iter().enumerate() {
            let mut pass = Pass::direct(
                &self.kernels.probe,
                figure.count() * PROBE_LANES,
                NARROW_WIDTH,
            )
            .read_at(&self.jobs, base)
            .uint(plan.probes + index as u32)
            .reads_through(&self.tables)
            .reads_through(&self.grid.cells)
            .reads_through(&self.grid.items)
            .reads_through(&self.grid.spheres)
            .reads_through(&self.grid.large)
            .writes_through(&figure.contacts)
            .writes_through(&figure.scratch)
            .reads_through(&figure.state)
            .reads_through(&figure.frame)
            .reads_through(&figure.normals);
            for other in figures {
                pass = pass
                    .reads_through(&other.posed)
                    .reads_through(&other.statics);
            }
            pass.run(batch);
        }
        self.tap(batch, "probe", Some(epoch), figures);
        for (index, figure) in figures.iter().enumerate() {
            Pass::direct(
                &self.kernels.wraps,
                figure.topology.triangles.len() as u32,
                NARROW_WIDTH,
            )
            .read_at(&self.jobs, base)
            .uint(plan.probes + index as u32)
            .reads_through(&figure.contacts)
            .reads_through(&figure.statics)
            .reads_through(&figure.posed)
            .writes_through(&figure.scratch)
            .run(batch);
        }
        self.tap(batch, "wraps", Some(epoch), figures);
    }

    fn solve(
        &self,
        batch: &mut Batch,
        figures: &[&Figure],
        plan: &Epoch,
        base: u64,
        measuring: bool,
    ) {
        let solves = &plan.solves;
        let mut start = 0;
        while start < solves.len() {
            let key = solves[start].key;
            let end = start
                + solves[start..]
                    .iter()
                    .take_while(|solve| solve.key == key)
                    .count();
            let mut pass = Pass::direct(self.solvers.kernel(key.2), (end - start) as u32, 1)
                .read_at(&self.jobs, base)
                .uint(plan.first + start as u32)
                .reads_through(&self.tables);
            if measuring {
                pass = pass.writes_through(&self.stats);
            }
            let mut visited: Vec<FigureId> = Vec::new();
            for solve in &solves[start..end] {
                if visited.contains(&solve.figure) {
                    continue;
                }
                visited.push(solve.figure);
                let figure = &self.figures[&solve.figure];
                pass = pass
                    .reads_through(&figure.frame)
                    .reads_through(&figure.normals)
                    .writes_through(&figure.state)
                    .writes_through(&figure.contacts)
                    .writes_through(&figure.scratch);
            }
            for other in figures {
                pass = pass
                    .reads_through(&other.posed)
                    .reads_through(&other.statics);
            }
            pass.run(batch);
            start = end;
        }
    }
}

fn framed(layout: FrameLayout, step_plan: &StepPlan) -> Vec<u8> {
    let mut bytes = vec![0; layout.step_plan as usize];
    for (at, section) in [
        (layout.palettes, bytemuck::cast_slice(&step_plan.palettes)),
        (layout.pieces, bytemuck::cast_slice(&step_plan.pieces)),
        (layout.colliders, bytemuck::cast_slice(&step_plan.colliders)),
        (layout.planes, bytemuck::cast_slice(&step_plan.planes)),
        (layout.grabs, bytemuck::cast_slice(&step_plan.grabs)),
    ] {
        let at = at as usize;
        bytes[at..at + section.len()].copy_from_slice(section);
    }
    bytes
}
