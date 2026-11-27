use std::ops::Range;
use std::sync::Arc;

use animate_body::{Body, HumanPose, Side, Worlds};
use animate_fit::Physique;
use rikka_model_core::Transform;
use rikka_model_core::glam::{Quat, Vec3};

use crate::PARALLEL;
use crate::evaluate::{Evaluator, settle};
use crate::jiggle::Jiggles;
use crate::motion::HumanMotion;
use crate::sample::PoseSample;
use crate::secondary::Springs;
use crate::tuning::CHUNK_FRAMES;

pub(super) type Unfitted = (HumanPose, [bool; 2]);

const FRAMES_PER_WORKER: usize = 32;

#[derive(Clone, Debug)]
pub(super) struct BakeFrame {
    pub(super) unfitted: Unfitted,
    pub(super) pose: HumanPose,
    pub(super) locals: Vec<Transform>,
    pub(super) lobes: Vec<Quat>,
}

#[derive(Debug)]
pub(super) struct Chunk {
    pub(super) springs: Springs,
    pub(super) jiggles: Jiggles,
    pub(super) frames: Vec<BakeFrame>,
}

pub(super) struct Evaluated {
    pub(super) unfitted: Vec<Unfitted>,
    pub(super) springs: Vec<Springs>,
    pub(super) after: Springs,
}

#[derive(Clone, Copy)]
pub(super) struct Bakery<'a> {
    pub(super) body: &'a Body,
    pub(super) physique: Option<&'a Physique>,
    pub(super) motion: &'a HumanMotion,
}

impl Bakery<'_> {
    pub(super) fn fit_reach(&self) -> u32 {
        match self.physique.is_some() && self.motion.settings.fitting {
            true => Physique::REACH,
            false => 0,
        }
    }

    pub(super) fn evaluate(
        &self,
        span: Range<u32>,
        length: u32,
        springs: Springs,
        eyes: &[Vec3],
    ) -> Evaluated {
        let body = self.body;
        let motion = self.motion;
        let settings = &motion.settings;
        let count = (span.end - span.start) as usize;
        let reach = settings.intensity.reach();
        let from = span.start.saturating_sub(reach);
        let to = (span.end + reach).min(length);
        let parallel = PARALLEL.grain(FRAMES_PER_WORKER);
        let mut pose_sample: Vec<PoseSample> = parallel.collect((to - from) as usize, |at| {
            motion.sample((from + at as u32) as f32, body.rest_pose())
        });
        settings
            .intensity
            .shape(body, &mut pose_sample, settings.limits);
        let lead = (span.start - from) as usize;
        let sampled = &pose_sample[lead..lead + count];
        let mut evaluator = Evaluator::resume(body, motion, springs);
        let mut checkpoints = Vec::new();
        let lagged: Vec<HumanPose> = sampled
            .iter()
            .enumerate()
            .map(|(at, pose_sample)| {
                if at % CHUNK_FRAMES as usize == 0 {
                    checkpoints.push(evaluator.springs().clone());
                }
                evaluator.lag(pose_sample)
            })
            .collect();
        let unfitted = parallel.collect(count, |at| {
            let pose_sample = &sampled[at];
            let arms_pinned =
                Side::BOTH.map(|side| pose_sample.pins[side.arm().index()].weight > 0.0);
            let eye = eyes[span.start as usize + at];
            (settle(body, lagged[at], pose_sample, || eye), arms_pinned)
        });
        Evaluated {
            unfitted,
            springs: checkpoints,
            after: evaluator.springs().clone(),
        }
    }

    pub(super) fn finish(
        &self,
        unfitted: &[Unfitted],
        first: u32,
        desired: Range<usize>,
        springs: Vec<Springs>,
        jiggles: &mut Jiggles,
    ) -> Vec<Arc<Chunk>> {
        let body = self.body;
        let fitted = self.fitted(first, unfitted, desired.clone());
        let (checkpoints, lobes) = self.jiggled(jiggles, &fitted);
        let locals = PARALLEL
            .grain(FRAMES_PER_WORKER)
            .collect(fitted.len(), |at| body.locals(&fitted[at]));
        let mut frames = unfitted[desired]
            .iter()
            .zip(fitted)
            .zip(locals)
            .zip(lobes)
            .map(|(((unfitted, pose), locals), lobes)| BakeFrame {
                unfitted: *unfitted,
                pose,
                locals,
                lobes,
            });
        springs
            .into_iter()
            .zip(checkpoints)
            .map(|(springs, jiggles)| {
                Arc::new(Chunk {
                    springs,
                    jiggles,
                    frames: frames.by_ref().take(CHUNK_FRAMES as usize).collect(),
                })
            })
            .collect()
    }

    fn fitted(&self, first: u32, unfitted: &[Unfitted], desired: Range<usize>) -> Vec<HumanPose> {
        match self.physique.filter(|_| self.motion.settings.fitting) {
            Some(physique) => physique.fit(self.body, first, unfitted, desired),
            None => unfitted[desired].iter().map(|(pose, _)| *pose).collect(),
        }
    }

    fn jiggled(
        &self,
        jiggles: &mut Jiggles,
        poses: &[HumanPose],
    ) -> (Vec<Jiggles>, Vec<Vec<Quat>>) {
        let body = self.body;
        let strength = self.motion.settings.jiggle;
        let chunk = CHUNK_FRAMES as usize;
        if strength <= 0.0 || body.lobes().is_empty() {
            let checkpoints = vec![jiggles.clone(); poses.len().div_ceil(chunk)];
            return (checkpoints, vec![Vec::new(); poses.len()]);
        }
        let surroundings: Vec<(Worlds, Vec<Vec3>)> =
            PARALLEL
                .grain(FRAMES_PER_WORKER)
                .collect(poses.len(), |at| {
                    let worlds = body.fk(&poses[at]);
                    let obstacles = self
                        .physique
                        .map(|physique| physique.reaching(&worlds).collect())
                        .unwrap_or_default();
                    (worlds, obstacles)
                });
        let mut checkpoints = Vec::with_capacity(poses.len().div_ceil(chunk));
        let lobes = surroundings
            .iter()
            .enumerate()
            .map(|(at, (worlds, obstacles))| {
                if at % chunk == 0 {
                    checkpoints.push(jiggles.clone());
                }
                jiggles.step(body, worlds, strength, obstacles)
            })
            .collect();
        (checkpoints, lobes)
    }
}
