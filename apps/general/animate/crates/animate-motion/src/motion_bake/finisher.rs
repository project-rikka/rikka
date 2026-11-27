use std::ops::Range;
use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;

use animate_body::Body;
use animate_fit::Physique;
use rikka_model_core::glam::Vec3;

use super::Slot;
use super::bakery::{Bakery, Evaluated};
use super::progress::Progress;
use crate::jiggle::Jiggles;
use crate::motion::HumanMotion;
use crate::tuning::CHUNK_FRAMES;

pub(super) const FIRST_PIECE_CHUNKS: usize = 4;

pub(super) struct Finisher {
    pub(super) body: Arc<Body>,
    pub(super) physique: Option<Arc<Physique>>,
    pub(super) motion: HumanMotion,
    pub(super) eyes: Arc<[Vec3]>,
    pub(super) head: Evaluated,
    pub(super) slots: Vec<Arc<Slot>>,
    pub(super) progress: Arc<Progress>,
}

impl Finisher {
    pub(super) fn run(mut self) {
        if panic::catch_unwind(AssertUnwindSafe(|| self.pieces())).is_err() {
            for slot in &self.slots {
                slot.get_or_init(|| Err(super::Failed));
            }
            self.progress.advance();
        }
    }

    fn bakery(&self) -> Bakery<'_> {
        Bakery {
            body: &self.body,
            physique: self.physique.as_deref(),
            motion: &self.motion,
        }
    }

    fn pieces(&mut self) {
        let count = self.slots.len();
        let first = FIRST_PIECE_CHUNKS.min(count);
        let mut jiggles = Jiggles::default();
        if !self.publish(0..first, &mut jiggles) || self.is_abandoned(first) {
            return;
        }
        let length = self.eyes.len() as u32;
        let evaluated = self.head.unfitted.len() as u32;
        if evaluated < length {
            let rest = self.bakery().evaluate(
                evaluated..length,
                length,
                self.head.after.clone(),
                &self.eyes,
            );
            self.head.unfitted.extend(rest.unfitted);
            self.head.springs.extend(rest.springs);
        }
        self.publish(first..count, &mut jiggles);
    }

    fn is_abandoned(&self, from: usize) -> bool {
        self.slots[from..]
            .iter()
            .all(|slot| Arc::strong_count(slot) == 1)
    }

    fn publish(&self, chunks: Range<usize>, jiggles: &mut Jiggles) -> bool {
        if chunks.is_empty() || self.is_abandoned(chunks.start) {
            return false;
        }
        let bakery = self.bakery();
        let unfitted = &self.head.unfitted;
        let reach = bakery.fit_reach() as usize;
        let chunk = CHUNK_FRAMES as usize;
        let frames = chunks.start * chunk..(chunks.end * chunk).min(self.eyes.len());
        let context = frames.start.saturating_sub(reach)..(frames.end + reach).min(unfitted.len());
        let baked = bakery.finish(
            &unfitted[context.clone()],
            context.start as u32,
            frames.start - context.start..frames.end - context.start,
            self.head.springs[chunks.clone()].to_vec(),
            jiggles,
        );
        for (slot, chunk) in self.slots[chunks].iter().zip(baked) {
            slot.get_or_init(|| Ok(chunk));
        }
        self.progress.advance();
        true
    }
}
