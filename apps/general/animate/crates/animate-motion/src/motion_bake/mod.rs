mod bakery;
mod finisher;
mod progress;

use std::ops::Range;
use std::sync::{Arc, OnceLock};
use std::thread;

use animate_body::{Body, HumanPose};
use animate_fit::Physique;
use rikka_model_core::glam::Vec3;
use rikka_model_core::{Pose, Transform};

use crate::jiggle::settling_frames;
use crate::motion::HumanMotion;
use crate::secondary::Springs;
use crate::tuning::{BAKE_TAIL_FRAMES, CHUNK_FRAMES, FRAMES_PER_SECOND, SECONDARY_TAIL_FRAMES};

use bakery::{BakeFrame, Bakery, Chunk, Unfitted};
use finisher::{FIRST_PIECE_CHUNKS, Finisher};
use progress::Progress;

type Slot = OnceLock<Result<Arc<Chunk>, Failed>>;

const BAKE_THREAD: &str = "rikka-bake";

#[derive(Clone, Copy, Debug)]
struct Failed;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Written {
    Baked,
    Estimated,
}

#[derive(Clone, Debug)]
pub struct MotionBake {
    body: Arc<Body>,
    physique: Option<Arc<Physique>>,
    motion: HumanMotion,
    eyes: Arc<[Vec3]>,
    chunks: Vec<Arc<Slot>>,
    progress: Arc<Progress>,
    length: u32,
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
        let eyes: Arc<[Vec3]> = (0..length).map(eye).collect();
        Self::baked(body, physique, motion, eyes)
    }

    fn baked(
        body: Arc<Body>,
        physique: Option<Arc<Physique>>,
        motion: &HumanMotion,
        eyes: Arc<[Vec3]>,
    ) -> Self {
        let length = eyes.len() as u32;
        let bakery = Bakery {
            body: &body,
            physique: physique.as_deref(),
            motion,
        };
        let head = match motion.settings.intensity.is_neutral() {
            true => (FIRST_PIECE_CHUNKS as u32 * CHUNK_FRAMES + bakery.fit_reach())
                .next_multiple_of(CHUNK_FRAMES)
                .min(length),
            false => length,
        };
        let head = bakery.evaluate(0..head, length, Springs::default(), &eyes);
        let slots: Vec<Arc<Slot>> = (0..length.div_ceil(CHUNK_FRAMES))
            .map(|_| Arc::default())
            .collect();
        let progress = Arc::new(Progress::default());
        let finisher = Finisher {
            body: Arc::clone(&body),
            physique: physique.clone(),
            motion: motion.clone(),
            eyes: Arc::clone(&eyes),
            head,
            slots: slots.clone(),
            progress: Arc::clone(&progress),
        };
        thread::Builder::new()
            .name(BAKE_THREAD.into())
            .spawn(move || finisher.run())
            .unwrap_or_else(|error| panic!("failed to spawn {BAKE_THREAD}: {error}"));
        Self {
            body,
            physique,
            motion: motion.clone(),
            eyes,
            chunks: slots,
            progress,
            length,
        }
    }

    #[must_use]
    pub fn rebaked(
        &self,
        motion: &HumanMotion,
        dirty: Range<u32>,
        eye: impl Fn(u32) -> Vec3,
    ) -> Self {
        let length = length_of(motion);
        let eyes: Arc<[Vec3]> = (0..length).map(eye).collect();
        let bakery = Bakery {
            body: &self.body,
            physique: self.physique.as_deref(),
            motion,
        };
        let fit = bakery.fit_reach();
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
        let after_end = match length == self.length {
            true => (end + fit).min(length),
            false => end,
        };
        let (Some(before), Some(after), Some(checkpoint)) = (
            self.unfitted_within(start.saturating_sub(fit)..start),
            self.unfitted_within(end..after_end),
            self.chunk(first as usize),
        ) else {
            return Self::baked(Arc::clone(&self.body), self.physique.clone(), motion, eyes);
        };
        if start == 0 && end == length {
            return Self::baked(Arc::clone(&self.body), self.physique.clone(), motion, eyes);
        }
        let evaluated = bakery.evaluate(start..end, length, checkpoint.springs.clone(), &eyes);
        let mut unfitted = before;
        let desired = unfitted.len()..unfitted.len() + evaluated.unfitted.len();
        unfitted.extend(evaluated.unfitted);
        unfitted.extend(after);
        let mut jiggles = checkpoint.jiggles.clone();
        let fresh = bakery.finish(
            &unfitted,
            start - desired.start as u32,
            desired,
            evaluated.springs,
            &mut jiggles,
        );
        let mut chunks: Vec<Arc<Slot>> = self.chunks[..first as usize].to_vec();
        chunks.extend(
            fresh
                .into_iter()
                .map(|chunk| Arc::new(OnceLock::from(Ok(chunk)))),
        );
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
            motion: motion.clone(),
            eyes,
            chunks,
            progress: Arc::clone(&self.progress),
            length,
        }
    }

    #[must_use]
    pub fn frames(&self) -> u32 {
        self.length
    }

    #[must_use]
    pub fn is_baked(&self, frame: u32) -> bool {
        self.frame(frame).is_some()
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.chunks.iter().all(|slot| slot.get().is_some())
    }

    #[must_use]
    pub fn baked_pose_at(&self, frame: u32) -> Option<&HumanPose> {
        self.frame(frame).map(|baked| &baked.pose)
    }

    #[must_use]
    pub fn pose_at(&self, frame: u32) -> HumanPose {
        match self.frame(frame) {
            Some(baked) => baked.pose,
            None => self.estimate(frame.min(self.length - 1) as f32),
        }
    }

    pub fn write(&self, seconds: f32, pose: &mut Pose) -> Written {
        let at = (seconds * FRAMES_PER_SECOND).max(0.0);
        let index = at.floor() as u32;
        let blend = at - at.floor();
        let next = match index + 1 >= self.length || blend <= 0.0 {
            true => index,
            false => index + 1,
        };
        let (Some(from), Some(to)) = (self.frame(index), self.frame(next)) else {
            let estimate = self.estimate(at.min((self.length - 1) as f32));
            self.body.write(&estimate, pose);
            return Written::Estimated;
        };
        let blend = if next == index { 0.0 } else { blend };
        let locals = from.locals.iter().zip(&to.locals);
        for (node, (one, two)) in self.body.driven_nodes().iter().zip(locals) {
            pose.set_local(*node, mixed(one, two, blend));
        }
        for ((lobe, one), two) in self.body.lobes().iter().zip(&from.lobes).zip(&to.lobes) {
            if let Some(local) = pose.local_mut(lobe.node) {
                local.rotation = (one.slerp(*two, blend) * local.rotation).normalize();
            }
        }
        Written::Baked
    }

    pub fn watch(&self, listener: &Arc<dyn Fn() + Send + Sync>) {
        self.progress.listen(|| self.is_complete(), listener);
    }

    pub fn wait(&self) {
        for slot in &self.chunks {
            slot.wait();
        }
    }

    fn estimate(&self, frame: f32) -> HumanPose {
        let eye = self.eyes[(frame.round() as usize).min(self.eyes.len() - 1)];
        self.motion
            .sample(frame, self.body.rest_pose())
            .reached(&self.body, || eye)
    }

    fn chunk(&self, index: usize) -> Option<&Chunk> {
        self.chunks
            .get(index)?
            .get()?
            .as_ref()
            .ok()
            .map(Arc::as_ref)
    }

    fn frame(&self, frame: u32) -> Option<&BakeFrame> {
        let frame = frame.min(self.length - 1);
        self.chunk((frame / CHUNK_FRAMES) as usize)
            .map(|chunk| &chunk.frames[(frame % CHUNK_FRAMES) as usize])
    }

    fn unfitted_within(&self, range: Range<u32>) -> Option<Vec<Unfitted>> {
        range
            .map(|frame| self.frame(frame).map(|baked| baked.unfitted))
            .collect()
    }
}

fn tail(motion: &HumanMotion) -> u32 {
    SECONDARY_TAIL_FRAMES.max(settling_frames(motion.settings.jiggle))
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
