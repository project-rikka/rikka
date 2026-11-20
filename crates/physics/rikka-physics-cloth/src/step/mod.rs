mod jobs;
mod rest;
mod round;

use std::sync::Arc;

use glam::Mat4;
use rikka_gpu::Batch;
use rikka_model_core::NodeId;

use crate::cloth::Cloth;
use crate::colliders::place;
use crate::drape::{Outputs, handedness};
use crate::error::{Error, Result};
use crate::figure::{Figure, FigureId};
use crate::grab;
use crate::inertia::{InertiaContext, jumped};
use crate::inputs::{Inputs, Joint, Posture};
use crate::layout::{ColliderRecord, FigureRecord, JobRecord, PieceRecord};
use crate::readback::{Movement, STATS_WORDS};
use crate::recorded::Recorded;
use crate::wind::blowing;

pub(crate) use rest::Rest;
pub(crate) use round::Environment;

const WARM_BLEND: u32 = 5;
const WARM_SETTLE: u32 = 45;
const TIME_PERIOD: f64 = 4096.0;
pub(crate) const JOB_BYTES: u64 = size_of::<JobRecord>() as u64;
pub(crate) const TABLE_BYTES: u64 = size_of::<FigureRecord>() as u64;
pub(crate) const MAX_FIGURES: usize = 32;

pub(crate) struct StepPlan {
    pub(crate) figure: FigureId,
    pub(crate) substeps: u32,
    pub(crate) palettes: Vec<Mat4>,
    pub(crate) pieces: Vec<PieceRecord>,
    pub(crate) colliders: Vec<ColliderRecord>,
    pub(crate) planes: Vec<[f32; 4]>,
    pub(crate) grabs: Vec<[f32; 4]>,
    pub(crate) grabbed: u32,
    pub(crate) still_damping: f32,
    pub(crate) activity: f32,
    pub(crate) stiffening: f32,
    pub(crate) freeze: f32,
}

impl Cloth {
    pub fn advance(&mut self, time: f64, inputs: &Inputs) -> Result<()> {
        let mut batch = self.compute.batch();
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
        self.validate(time, inputs)?;
        let (rounds, last) = self.plan(batch, inputs)?;
        let (wind, turbulence) = blowing(&inputs.wind, time);
        let environment = Environment {
            time: time.rem_euclid(TIME_PERIOD) as f32,
            wind,
            turbulence,
        };
        self.reserve(&rounds, &last)?;
        let mut cursors = (0, 0);
        for round in &rounds {
            self.record(batch, round, &environment, false, &mut cursors);
        }
        self.absorb(batch, inputs, &last);
        let lasting: Vec<&Figure> = last
            .iter()
            .map(|step_plan| &self.figures[&step_plan.figure])
            .collect();
        self.tap(batch, "absorb", None, &lasting);
        batch.fill(&self.stats, 0);
        self.record(batch, &last, &environment, true, &mut cursors);
        let shown: Vec<FigureId> = last.iter().map(|step_plan| step_plan.figure).collect();
        for step_plan in &last {
            if let Some(figure) = self.figures.get_mut(&step_plan.figure) {
                let boundary = self.settings.effort.epochs as usize;
                let slots = figure.topology.slots.len();
                figure.shown.clear();
                figure.shown.extend_from_slice(
                    &step_plan.palettes[boundary * slots..(boundary + 1) * slots],
                );
            }
        }
        let boundary = self.settings.effort.epochs;
        self.present(batch, &shown, boundary, false)?;
        let presented: Vec<&Figure> = shown.iter().map(|id| &self.figures[id]).collect();
        self.tap(batch, "present", None, &presented);
        let stats_bytes = (last.len() as u64 * STATS_WORDS * 4).max(4);
        let stats = batch.download_range(&self.stats, 0, stats_bytes);
        self.traced = self.tracer.as_ref().map(|tracer| tracer.finish(batch));
        self.timing = self.profiling.then(|| batch.timing());
        let measured = shown
            .iter()
            .map(|id| (*id, self.figures[id].count()))
            .collect();
        self.stats_readback.push(self.steps, stats, measured);
        self.substeps = last
            .iter()
            .map(|step_plan| step_plan.substeps)
            .max()
            .unwrap_or(0);
        self.steps += 1;
        Ok(Recorded { shown })
    }

    fn validate(&self, time: f64, inputs: &Inputs) -> Result<()> {
        if !time.is_finite() {
            return Err(Error::NonFinite("cloth time"));
        }
        for (id, posture) in &inputs.figures {
            let figure = self.figure(*id)?;
            let nodes = figure.asset.rest_worlds.len();
            if posture.worlds.len() != nodes {
                return Err(Error::PoseMismatch {
                    figure: *id,
                    expected: nodes,
                    actual: posture.worlds.len(),
                });
            }
            if !posture.transform.is_finite() || !posture.worlds.iter().all(Mat4::is_finite) {
                return Err(Error::NonFinitePose(*id));
            }
        }
        for (id, node, buffer) in &inputs.reactions {
            let figure = self.figure(*id)?;
            let Some(slot) = figure.topology.nodes.binary_search(node).ok() else {
                return Err(Error::UnknownReactionNode {
                    figure: *id,
                    node: *node,
                });
            };
            let vertices = figure
                .outputs
                .as_ref()
                .map_or(0, |outputs| outputs.vertices(slot));
            if buffer.size() < u64::from(vertices) * rikka_physics_gpu::REACTION_BYTES {
                return Err(Error::ReactionBufferTooSmall {
                    node: *node,
                    vertices,
                });
            }
        }
        for (id, _) in &inputs.grabs {
            self.figure(*id)?;
        }
        for (&id, figure) in &self.figures {
            let grabs = inputs.grabs_of(id);
            if !grabs.is_empty() {
                grab::validate(id, &grabs, &figure.asset)?;
            }
        }
        let posed = self
            .figures
            .iter()
            .filter(|(id, figure)| figure.posture.is_some() || inputs.posture(**id).is_some())
            .count();
        if posed > MAX_FIGURES {
            return Err(Error::TooManyFigures { count: posed });
        }
        Ok(())
    }

    fn plan(
        &mut self,
        batch: &mut Batch,
        inputs: &Inputs,
    ) -> Result<(Vec<Vec<StepPlan>>, Vec<StepPlan>)> {
        let warm = (WARM_BLEND + WARM_SETTLE) as usize;
        let mut rounds: Vec<Vec<StepPlan>> = (0..warm).map(|_| Vec::new()).collect();
        let mut last = Vec::new();
        let mut still = inputs.wind.speed <= 0.0;
        let gravity = self.settings.gravity;
        let dt = 1.0 / self.settings.rate;
        let effort = self.settings.effort;
        for (&id, figure) in &mut self.figures {
            if let Some(posture) = inputs.posture(id) {
                figure.posture = Some(posture.clone());
            }
            let Some(posture) = &figure.posture else {
                continue;
            };
            let transform = posture.transform;
            let current = joints(&figure.topology.slots, &figure.inverse_rest, posture);
            let grabs = inputs.grabs_of(id);
            still &= figure.grabbing.unchanged(&grabs);
            let grips = figure.grabbing.plan(&figure.topology, &grabs);
            if !grips.entries.is_empty() {
                batch.write(
                    &figure.frame,
                    figure.frame_layout.grabbed,
                    bytemuck::cast_slice(&grips.entries),
                );
            }
            if figure.outputs.is_none() {
                let model = Arc::clone(&posture.model);
                figure.outputs = Some(Outputs::new(&model, &figure.topology.nodes, &figure.bound)?);
                let hands = handedness(&model, &figure.bound);
                if !hands.is_empty() {
                    batch.write(&figure.hands, 0, bytemuck::cast_slice(&hands));
                }
            }
            let inertia_context =
                |figure: &mut Figure, from: &[Joint], to: &[Joint], warming: Option<[f32; 2]>| {
                    let Figure {
                        topology,
                        styles,
                        motion,
                        colliders,
                        ..
                    } = figure;
                    let inertia_context = InertiaContext {
                        topology,
                        styles,
                        gravity,
                        dt,
                        carried_whole: warming.is_some(),
                    };
                    let epochs = effort.epochs;
                    let substeps = inertia_context.substeps(from, to, effort.substeps, epochs);
                    let mut pieces = inertia_context.pieces(from, to, &mut motion.smoothed);
                    for (record, grab) in pieces.iter_mut().zip(&grips.pieces) {
                        record.grab[0] = *grab;
                    }
                    let mut palettes = Vec::with_capacity(from.len() * (epochs as usize + 1));
                    let mut placed = Vec::new();
                    let mut planes = Vec::new();
                    for boundary in 0..=epochs {
                        let share = boundary as f32 / epochs as f32;
                        let blended = blend(from, to, share);
                        palettes.extend(blended.iter().map(Joint::matrix));
                        place(colliders, &blended, &mut placed, &mut planes);
                    }
                    StepPlan {
                        figure: id,
                        substeps,
                        palettes,
                        pieces,
                        colliders: placed,
                        planes,
                        grabs: grips.slots(epochs, warming),
                        grabbed: grips.entries.len() as u32,
                        still_damping: 0.0,
                        activity: 1.0,
                        stiffening: 0.0,
                        freeze: 0.0,
                    }
                };
            if figure
                .motion
                .joints
                .as_ref()
                .is_some_and(|previous| jumped(&figure.topology, previous, &current, dt))
            {
                figure.motion.fresh = true;
            }
            if figure.motion.fresh {
                figure.motion.fresh = false;
                still = false;
                start(batch, figure, transform);
                let bind = vec![Joint::from_matrix(transform); current.len()];
                for (step, round) in rounds.iter_mut().enumerate() {
                    let share = |at: usize| (at as f32 / WARM_BLEND as f32).min(1.0);
                    let from: Vec<Joint> = blend(&bind, &current, share(step));
                    let to: Vec<Joint> = blend(&bind, &current, share(step + 1));
                    let warming = [share(step), share(step + 1)];
                    round.push(inertia_context(figure, &from, &to, Some(warming)));
                }
                figure.motion.joints = Some(current.clone());
            }
            let previous = figure
                .motion
                .joints
                .take()
                .unwrap_or_else(|| current.clone());
            still &= previous == current;
            last.push(inertia_context(figure, &previous, &current, None));
            figure.motion.joints = Some(current);
        }
        rounds.retain(|round| !round.is_empty());
        let movement = self.movement(still)?;
        self.rest.observe(still, movement);
        if self.rest.asleep() {
            last.clear();
        }
        let still_damping = self.rest.damping(movement.speed);
        let activity = self.rest.activity();
        let stiffening = self.rest.stiffening();
        let freeze = self.rest.freeze();
        for step_plan in &mut last {
            step_plan.still_damping = still_damping;
            step_plan.activity = activity;
            step_plan.stiffening = stiffening;
            step_plan.freeze = freeze;
        }
        Ok((rounds, last))
    }

    fn movement(&mut self, still: bool) -> Result<Movement> {
        if still && !self.rest.asleep() {
            return self.stats_readback.lagged(self.steps);
        }
        self.stats_readback.drain(self.steps)?;
        Ok(Movement::default())
    }

    fn reserve(&mut self, rounds: &[Vec<StepPlan>], last: &[StepPlan]) -> Result<()> {
        let epochs = u64::from(self.settings.effort.epochs);
        let jobs: u64 = rounds
            .iter()
            .flatten()
            .chain(last)
            .map(|step_plan| {
                (self.figures[&step_plan.figure].topology.groups.len() as u64 + 1) * epochs
            })
            .sum();
        if jobs * JOB_BYTES > self.jobs.size() {
            self.jobs = self
                .compute
                .buffer((jobs * JOB_BYTES).next_power_of_two())?;
        }
        let tables: u64 = rounds
            .iter()
            .map(Vec::len)
            .chain(std::iter::once(last.len()))
            .map(|figures| figures as u64 * epochs * TABLE_BYTES)
            .sum();
        if tables > self.tables.size() {
            self.tables = self.compute.buffer(tables.next_power_of_two())?;
        }
        let obstacles: u64 = last
            .iter()
            .map(|step_plan| {
                let figure = &self.figures[&step_plan.figure];
                u64::from(figure.body_triangles()) + figure.topology.triangles.len() as u64
            })
            .sum();
        self.grid.reserve(&self.compute, obstacles)?;
        let stats = (last.len() as u64 * STATS_WORDS * 4).max(4);
        if stats > self.stats.size() {
            self.stats = self.compute.buffer(stats.next_power_of_two())?;
        }
        Ok(())
    }
}

fn joints(slots: &[NodeId], inverse_rest: &[Mat4], posture: &Posture) -> Vec<Joint> {
    slots
        .iter()
        .zip(inverse_rest)
        .map(|(node, inverse)| {
            Joint::from_matrix(posture.transform * posture.worlds[node.index()] * *inverse)
        })
        .collect()
}

fn blend(from: &[Joint], to: &[Joint], share: f32) -> Vec<Joint> {
    from.iter()
        .zip(to)
        .map(|(start, end)| start.lerp(end, share))
        .collect()
}

fn start(batch: &mut Batch, figure: &Figure, transform: Mat4) {
    let positions: Vec<[f32; 4]> = figure
        .topology
        .order
        .iter()
        .map(|&(piece, particle)| {
            transform
                .transform_point3(figure.asset.pieces[piece].particles[particle].rest)
                .extend(0.0)
                .to_array()
        })
        .collect();
    batch.write(&figure.state, 0, bytemuck::cast_slice(&positions));
    batch.fill_range(
        &figure.state,
        figure.velocities(),
        2 * figure.velocities(),
        0,
    );
    batch.fill(&figure.normals, 0);
    batch.fill(&figure.contacts, u32::MAX);
}
