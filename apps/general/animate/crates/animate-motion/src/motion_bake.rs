use std::ops::Range;
use std::sync::Arc;

use animate_body::{Body, HumanPose, Side};
use animate_fit::Physique;
use rikka_model_core::glam::{Quat, Vec3};
use rikka_model_core::{Pose, Transform};

use crate::evaluate::Evaluator;
use crate::jiggle::{Jiggles, settling_frames};
use crate::motion::HumanMotion;
use crate::secondary::Springs;
use crate::tuning::{BAKE_TAIL_FRAMES, CHUNK_FRAMES, FRAMES_PER_SECOND, SECONDARY_TAIL_FRAMES};

type Unfitted = (HumanPose, [bool; 2]);

fn tail(motion: &HumanMotion) -> u32 {
    SECONDARY_TAIL_FRAMES.max(settling_frames(motion.settings.jiggle))
}

#[derive(Clone, Debug)]
struct BakeFrame {
    unfitted: Unfitted,
    pose: HumanPose,
    locals: Vec<Transform>,
    lobes: Vec<Quat>,
}

#[derive(Debug)]
struct Chunk {
    springs: Springs,
    jiggles: Jiggles,
    frames: Vec<BakeFrame>,
}

#[derive(Clone, Debug)]
pub struct MotionBake {
    body: Arc<Body>,
    physique: Option<Arc<Physique>>,
    chunks: Vec<Arc<Chunk>>,
    length: u32,
}

struct Span<'a> {
    start: u32,
    end: u32,
    length: u32,
    springs: Springs,
    jiggles: Jiggles,
    before: &'a [Unfitted],
    after: &'a [Unfitted],
}

impl MotionBake {
    #[must_use]
    pub fn new(
        body: Arc<Body>,
        physique: Option<Arc<Physique>>,
        motion: &HumanMotion,
        eye: impl Fn(u32) -> Vec3,
    ) -> Self {
        let length = length_of(motion);
        let mut motion_bake = Self {
            body,
            physique,
            chunks: Vec::new(),
            length,
        };
        motion_bake.chunks = motion_bake.bake(
            motion,
            Span {
                start: 0,
                end: length,
                length,
                springs: Springs::default(),
                jiggles: Jiggles::default(),
                before: &[],
                after: &[],
            },
            &eye,
        );
        motion_bake
    }

    #[must_use]
    pub fn rebaked(
        &self,
        motion: &HumanMotion,
        dirty: Range<u32>,
        eye: impl Fn(u32) -> Vec3,
    ) -> Self {
        let length = length_of(motion);
        let fit = self.fit_reach(motion);
        let lead = motion.settings.intensity.reach() + fit;
        let first = (dirty.start.saturating_sub(lead) / CHUNK_FRAMES)
            .min(self.chunks.len() as u32 - 1)
            .min((length - 1) / CHUNK_FRAMES);
        let stable_end = dirty
            .end
            .saturating_sub(1)
            .max(dirty.start)
            .saturating_add(tail(motion) + lead);
        let through = match length == self.length {
            true => stable_end.min(length - 1),
            false => length - 1,
        };
        let start = first * CHUNK_FRAMES;
        let end = ((through / CHUNK_FRAMES + 1) * CHUNK_FRAMES).min(length);
        let before = self.unfitted_within(start.saturating_sub(fit)..start);
        let after = match length == self.length {
            true => self.unfitted_within(end..(end + fit).min(length)),
            false => Vec::new(),
        };
        let checkpoint = &self.chunks[first as usize];
        let fresh = self.bake(
            motion,
            Span {
                start,
                end,
                length,
                springs: checkpoint.springs.clone(),
                jiggles: checkpoint.jiggles.clone(),
                before: &before,
                after: &after,
            },
            &eye,
        );
        let mut chunks: Vec<Arc<Chunk>> = self.chunks[..first as usize].to_vec();
        chunks.extend(fresh);
        chunks.extend(
            self.chunks
                .iter()
                .skip(chunks.len())
                .take((length.div_ceil(CHUNK_FRAMES) as usize).saturating_sub(chunks.len()))
                .cloned(),
        );
        Self {
            body: Arc::clone(&self.body),
            physique: self.physique.clone(),
            chunks,
            length,
        }
    }

    #[must_use]
    pub fn frames(&self) -> u32 {
        self.length
    }

    #[must_use]
    pub fn pose_at(&self, frame: u32) -> &HumanPose {
        &self.frame(frame).pose
    }

    pub fn write(&self, seconds: f32, pose: &mut Pose) {
        let (from, to, blend) = self.bracket(seconds);
        let locals = from.locals.iter().zip(&to.locals);
        for (node, (one, two)) in self.body.driven_nodes().iter().zip(locals) {
            pose.set_local(*node, mixed(one, two, blend));
        }
        for ((lobe, one), two) in self.body.lobes().iter().zip(&from.lobes).zip(&to.lobes) {
            if let Some(local) = pose.local_mut(lobe.node) {
                local.rotation = (one.slerp(*two, blend) * local.rotation).normalize();
            }
        }
    }

    fn fit_reach(&self, motion: &HumanMotion) -> u32 {
        match self.physique.is_some() && motion.settings.fitting {
            true => Physique::REACH,
            false => 0,
        }
    }

    fn unfitted_within(&self, range: Range<u32>) -> Vec<Unfitted> {
        range.map(|frame| self.frame(frame).unfitted).collect()
    }

    fn bracket(&self, seconds: f32) -> (&BakeFrame, &BakeFrame, f32) {
        let at = (seconds * FRAMES_PER_SECOND).max(0.0);
        let index = at.floor() as u32;
        let blend = at - at.floor();
        let from = self.frame(index);
        if index + 1 >= self.length || blend <= 0.0 {
            return (from, from, 0.0);
        }
        (from, self.frame(index + 1), blend)
    }

    fn frame(&self, frame: u32) -> &BakeFrame {
        let frame = frame.min(self.length - 1);
        let chunk = &self.chunks[(frame / CHUNK_FRAMES) as usize];
        &chunk.frames[(frame % CHUNK_FRAMES) as usize]
    }

    fn bake(
        &self,
        motion: &HumanMotion,
        span: Span<'_>,
        eye: &impl Fn(u32) -> Vec3,
    ) -> Vec<Arc<Chunk>> {
        let body = self.body.as_ref();
        let settings = &motion.settings;
        let count = (span.end - span.start) as usize;
        let reach = settings.intensity.reach();
        let from = span.start.saturating_sub(reach);
        let to = (span.end + reach).min(span.length);
        let mut pose_sample: Vec<_> = (from..to)
            .map(|frame| motion.sample(frame as f32, body.rest_pose()))
            .collect();
        settings
            .intensity
            .shape(body, &mut pose_sample, settings.limits);
        let lead = (span.start - from) as usize;
        let mut evaluator = Evaluator::resume(body, motion, span.springs);
        let mut springs = Vec::new();
        let evaluated: Vec<Unfitted> = pose_sample
            .into_iter()
            .skip(lead)
            .take(count)
            .enumerate()
            .map(|(at, pose_sample)| {
                if at % CHUNK_FRAMES as usize == 0 {
                    springs.push(evaluator.springs().clone());
                }
                let arms_pinned =
                    Side::BOTH.map(|side| pose_sample.pins[side.arm().index()].weight > 0.0);
                let frame = span.start + at as u32;
                (
                    evaluator.evaluate_sampled(pose_sample, eye(frame)),
                    arms_pinned,
                )
            })
            .collect();
        let mut unfitted: Vec<Unfitted> = span.before.to_vec();
        unfitted.extend(evaluated);
        unfitted.extend_from_slice(span.after);
        let desired = span.before.len()..span.before.len() + count;
        let first = span.start - span.before.len() as u32;
        let fitted = self.fitted(motion, first, &unfitted, desired.clone());
        let mut jiggles = span.jiggles;
        let mut checkpoints = Vec::new();
        let frames: Vec<BakeFrame> = unfitted[desired]
            .iter()
            .zip(fitted)
            .enumerate()
            .map(|(at, (unfitted, pose))| {
                if at % CHUNK_FRAMES as usize == 0 {
                    checkpoints.push(jiggles.clone());
                }
                let lobes = match settings.jiggle > 0.0 && !body.lobes().is_empty() {
                    true => {
                        let worlds = body.fk(&pose);
                        let obstacles: Vec<Vec3> = self
                            .physique
                            .as_ref()
                            .map(|physique| physique.reaching(&worlds).collect())
                            .unwrap_or_default();
                        jiggles.step(body, &worlds, settings.jiggle, &obstacles)
                    }
                    false => Vec::new(),
                };
                BakeFrame {
                    unfitted: *unfitted,
                    locals: body.locals(&pose),
                    lobes,
                    pose,
                }
            })
            .collect();
        let mut frames = frames.into_iter();
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

    fn fitted(
        &self,
        motion: &HumanMotion,
        first: u32,
        unfitted: &[Unfitted],
        desired: Range<usize>,
    ) -> Vec<HumanPose> {
        match self.physique.as_ref().filter(|_| motion.settings.fitting) {
            Some(physique) => physique.fit(&self.body, first, unfitted, desired),
            None => unfitted[desired].iter().map(|(pose, _)| *pose).collect(),
        }
    }
}

fn mixed(one: &Transform, two: &Transform, blend: f32) -> Transform {
    if blend <= 0.0 {
        return *one;
    }
    Transform {
        translation: one.translation.lerp(two.translation, blend),
        rotation: one.rotation.slerp(two.rotation, blend),
        scale: one.scale.lerp(two.scale, blend),
    }
}

fn length_of(motion: &HumanMotion) -> u32 {
    motion.last_frame() + BAKE_TAIL_FRAMES + 1
}
