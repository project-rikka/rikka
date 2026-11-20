use glam::Vec3;
use rikka_gpu::{Batch, ComputeBuffer};

use crate::builder::BINS_PER_PARTICLE;
use crate::config::FASTEST_STEP;
use crate::error::{Error, Result};
use crate::fluid::Fluid;
use crate::frames::Stats;
use crate::gpu::settings::{Grid, NARROWEST_GRID, SettingsBlock};
use crate::gpu::{BodyDrape, FoamInputs, SpawnInputs, StepInputs, Substep, spawn, substep};
use crate::inputs::Inputs;
use crate::random::Random;
use crate::recorded::Recorded;
use crate::step::spawn::Window;
use crate::step::{Nozzles, Plan, SpawnList};
use crate::substance::Substance;

const GRID_MARGIN: f32 = 4.0;
const GRID_SLACK: f32 = 1.1;
const MIN_LIVE_BINS: u32 = 1 << 10;

struct LiveCounts {
    live: u32,
    colours: u32,
    spray: u32,
    has_spray: bool,
}

impl Fluid {
    pub fn advance_to(&mut self, time: f64, inputs: &Inputs) -> Result<()> {
        let mut batch = self.stage.compute.batch();
        let recorded = self.advance_into(&mut batch, time, inputs)?;
        let ready = batch.submit()?;
        self.commit(recorded, &ready);
        Ok(())
    }

    pub fn advance_into(
        &mut self,
        batch: &mut Batch,
        time: f64,
        inputs: &Inputs,
    ) -> Result<Recorded> {
        if !time.is_finite() {
            return Err(Error::NonFinite("fluid time"));
        }
        self.validate(inputs)?;
        self.stats_readback.settle(&mut self.clock.measured, 1)?;
        let basis = self.clock.measured.basis(self.stats_readback.len());
        let start = self.clock.time.unwrap_or(time);
        if time < start {
            return Err(Error::Backwards {
                from: start,
                to: time,
            });
        }
        let span = time - start;
        self.pose_bodies(inputs);
        let plan = self.plan(span, inputs, basis.fastest);
        let spawns = self.spawn_book(inputs, start, span, plan);
        let appended = self.clock.slots;
        let live_now = basis
            .live
            .saturating_add(u32::try_from(self.clock.spawned - basis.spawned).unwrap_or(u32::MAX));
        let live_bound = live_now.saturating_add(spawns.records.len() as u32);
        self.stage.grid = self.grid_for(time, inputs, &basis, live_bound);
        self.write_settings(batch);
        self.stage.bodies.note_entries(basis.entries);
        self.stage.bodies.grow_entries(&self.stage.compute)?;
        self.stage
            .bodies
            .pose(batch, &self.stage.kernels, &self.stage.settings);
        let spawn_buffer = if spawns.records.is_empty() {
            None
        } else {
            Some(
                self.spawn_records
                    .hold(&self.stage.compute, batch, spawns.bytes())?,
            )
        };
        let has_spray = self.foam.capacity > 0 && self.substances.iter().any(Substance::sprays);
        let live_spray = if self.clock.sprayed {
            self.spray_bound(basis.foam, appended)
        } else {
            basis.foam.min(self.foam.capacity)
        };
        let live_counts = LiveCounts {
            live: live_now,
            colours: basis.colours,
            spray: live_spray,
            has_spray,
        };
        if self.stage.bodies.has_drapes() {
            self.stage.bodies.clear_reactions(batch);
        }
        self.run(
            batch,
            plan,
            span,
            &spawns,
            spawn_buffer.as_ref(),
            live_counts,
        );
        self.clock.moved = match plan.substeps {
            0 => self.clock.moved && self.clock.slots == appended,
            _ => true,
        };
        let keep = (-f64::from(inputs.drying) * span).exp() as f32;
        self.stage.bodies.dry(batch, &self.stage.kernels, keep);
        self.stage.bodies.react_out(batch, &self.stage.compute)?;
        let spawned_spray = has_spray && plan.substeps > 0;
        let candidates = (if self.clock.sprayed { appended } else { 0 })
            .saturating_add(if spawned_spray { self.clock.slots } else { 0 });
        let spray = self.spray_bound(basis.foam, candidates);
        self.clock.sprayed = spawned_spray;
        let frame = self.render_state(batch, spray)?;
        self.resident_spray = spray;
        self.measure(batch);
        let stats = batch.download_buffer(&self.stats);
        self.timing = self.profiling.then(|| batch.timing());
        self.stats_readback.push(stats, time, self.clock.spawned);
        self.clock.time = Some(time);
        self.clock.steps += u64::from(plan.substeps);
        self.clock.substeps = plan.substeps;
        Ok(Recorded { frame })
    }

    pub(crate) fn write_settings(&mut self, batch: &mut Batch) {
        if self.settings_dirty {
            let block = self.stage.config.block(
                &self.substances,
                self.stage.bodies.active_grid(),
                self.stage.gravity,
            );
            batch.write(&self.stage.settings, 0, bytemuck::bytes_of(&block));
            self.settings_dirty = false;
        }
        let grid = self.stage.grid.block(true);
        batch.write(
            &self.stage.settings,
            std::mem::offset_of!(SettingsBlock, grid) as u64,
            bytemuck::bytes_of(&grid),
        );
    }

    fn pose_bodies(&mut self, inputs: &Inputs) {
        for body in &mut self.stage.bodies.list {
            body.previous.clone_from(&body.palette);
            body.previous_morph_weights.clone_from(&body.morph_weights);
            let Some((_, posture)) = inputs.bodies.iter().find(|(id, _)| *id == body.id) else {
                continue;
            };
            body.palette.clone_from(&posture.palette);
            body.morph_weights.clone_from(&posture.morph_weights);
            body.drapes.clear();
            body.drapes.extend(
                inputs
                    .drapes
                    .iter()
                    .filter(|draped| draped.body == body.id)
                    .filter_map(|draped| {
                        let part = body.mesh.part(draped.node)?;
                        Some(BodyDrape {
                            node: draped.node,
                            first_vertex: part.first_vertex,
                            vertices: part.vertices,
                            buffer: draped.buffer.clone(),
                        })
                    }),
            );
        }
    }

    fn plan(&self, span: f64, inputs: &Inputs, measured: f32) -> Plan {
        if span <= 0.0 {
            return Plan::new(0.0, 0.0, 1.0, self.stage.config.substeps);
        }
        let nozzles = inputs
            .emissions
            .iter()
            .map(|emission| {
                emission.velocity.length() + emission.from.distance(emission.to) / span as f32
            })
            .fold(0.0, f32::max);
        let bodies = self.stage.bodies.travel() / span as f32;
        let fastest = measured.max(nozzles).max(bodies);
        let plan = Plan::new(
            span,
            fastest,
            self.stage.config.cfl * self.stage.config.h,
            self.stage.config.substeps,
        );
        let longest = self.stage.config.longest_step(&self.substances);
        if f64::from(plan.dt) > f64::from(longest) {
            Plan::new(
                span,
                0.0,
                1.0,
                (
                    ((span / f64::from(longest)).ceil() as u32).min(self.stage.config.substeps.1),
                    self.stage.config.substeps.1,
                ),
            )
        } else {
            plan
        }
    }

    fn grid_for(&self, time: f64, inputs: &Inputs, basis: &Stats, live: u32) -> Grid {
        let cap = live
            .saturating_mul(BINS_PER_PARTICLE)
            .clamp(MIN_LIVE_BINS, self.stage.config.grid.bins())
            .next_power_of_two()
            .min(self.stage.config.grid.bins());
        let Some([low, high]) = basis.bounds else {
            return Grid::sized(cap, NARROWEST_GRID);
        };
        let (low, high) = inputs
            .emissions
            .iter()
            .flat_map(|emission| [emission.from, emission.to])
            .fold((low, high), |(low, high), point| {
                (low.min(point), high.max(point))
            });
        let travel = 2.0 * basis.fastest * (time - basis.time) as f32;
        let cells = (high - low + Vec3::splat(2.0 * travel)) / self.stage.config.h
            + Vec3::splat(GRID_MARGIN);
        self.stage
            .config
            .grid
            .fitted((cells * GRID_SLACK).to_array(), cap)
    }

    fn spawn_book(&mut self, inputs: &Inputs, start: f64, span: f64, plan: Plan) -> SpawnList {
        let bodies = &self.stage.bodies;
        let lookup = |region| bodies.region(region);
        let nozzles = Nozzles {
            spacing: self.stage.config.spacing,
            volume: self.stage.config.volume,
            lift: self.stage.config.thickness * 1.05,
            region: &lookup,
        };
        let window = Window {
            start,
            span,
            substeps: plan.substeps,
            dt: f64::from(plan.dt),
        };
        let room = self.stage.config.capacity - self.clock.slots.min(self.stage.config.capacity);
        SpawnList::fill(
            &inputs.emissions,
            &window,
            &nozzles,
            &mut self.clock.carries,
            room,
        )
    }

    fn run(
        &mut self,
        batch: &mut Batch,
        plan: Plan,
        span: f64,
        spawns: &SpawnList,
        buffer: Option<&ComputeBuffer>,
        live_counts: LiveCounts,
    ) {
        let viscosity = self
            .substances
            .iter()
            .map(|substance| substance.kinematic_viscosity())
            .fold(0.0, f32::max);
        let bonds = self.substances.iter().any(Substance::bonds);
        let cohesive = self.substances.iter().any(Substance::cohesive);
        let has_drapes = self.stage.bodies.has_drapes();
        if bonds && !self.clock.bonds_active {
            batch.fill(&self.particles.state().bond_counts, 0);
            self.clock.bonds_active = true;
        } else if !bonds {
            self.clock.bonds_active = false;
        }
        let collide = self.stage.config.floor.is_some() || self.stage.bodies.buffers.is_some();
        let inverse_span = if span > 0.0 { (1.0 / span) as f32 } else { 0.0 };
        let capacity = self.stage.config.capacity;
        let mut slots = self.clock.slots;
        let mut live = Some(live_counts.live);
        let mut pending = None;
        let step_inputs = StepInputs {
            kernels: &self.stage.kernels,
            viscous: &self.viscous,
            settings: &self.stage.settings,
            sorter: &self.sorter,
            grid: self.stage.grid,
            bodies: self.stage.bodies.buffers.as_ref(),
        };
        let rounds = plan.substeps.max(1);
        for round in 0..rounds {
            let (first, requested) = spawns.ranges.get(round as usize).copied().unwrap_or((0, 0));
            let count = requested.min(capacity - slots);
            if let (Some(spawns), true) = (buffer, count > 0) {
                let spawn_inputs = SpawnInputs {
                    spawns,
                    first,
                    count,
                    base: slots,
                    alpha: match plan.substeps {
                        0 => 1.0,
                        n => round as f32 / n as f32,
                    },
                    inverse_span,
                };
                spawn(&step_inputs, &self.particles, batch, &spawn_inputs);
            }
            slots += count;
            self.clock.spawned += u64::from(count);
            if plan.substeps == 0 {
                break;
            }
            let sorted = slots;
            let alive = match live.take() {
                Some(bound) => (bound + count).min(slots),
                None => slots,
            };
            let n = plan.substeps as f32;
            let step = Substep {
                sorted,
                live: alive,
                dt: plan.dt,
                alpha: (round as f32 / n, (round + 1) as f32 / n),
                gravity: self.stage.gravity,
                iterations: self.stage.config.iterations,
                sweeps: self.stage.config.sweeps(plan.dt, viscosity),
                bonds,
                cohesive,
                colours: live_counts.colours,
                collide,
                has_drapes,
                potentials: round + 1 == plan.substeps && live_counts.has_spray,
                shown: round + 1 == plan.substeps,
                seed: seed(self.clock.steps + u64::from(round)),
                max_speed: FASTEST_STEP * self.stage.config.h / plan.dt,
                spacing: self.stage.config.spacing,
            };
            pending = substep(&step_inputs, &mut self.particles, batch, &step, pending);
            slots = alive;
        }
        if let Some(pending) = pending {
            pending.finish(&step_inputs, &self.particles, batch);
        }
        if plan.substeps > 0 && (live_counts.has_spray || live_counts.spray > 0) {
            let foam_inputs = FoamInputs {
                settings: &self.stage.settings,
                particles: &self.particles,
                starts: &self.sorter.starts,
                bodies: self.stage.bodies.buffers.as_ref(),
                count: if live_counts.has_spray { slots } else { 0 },
                resident: live_counts.spray,
                dt: span as f32,
                gravity: self.stage.gravity,
                seed: seed(self.clock.steps ^ 0x5eed),
            };
            self.foam.step(batch, &self.stage.kernels, &foam_inputs);
        }
        self.clock.slots = slots;
    }
}

fn seed(step: u64) -> u32 {
    (Random::new(&[step]).next() >> 32) as u32
}
