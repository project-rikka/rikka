mod emitter_state;
mod grid;
mod liquid_scene;

use animate_cast::LiquidCache;
pub use grid::frame_at_step;
pub(crate) use grid::{step_of, step_of_frame};
pub(crate) use liquid_scene::{CastMember, LiquidScene};
use rikka_physics::{Bake, Progress, ProgressCallback};

const CACHE_BUDGET: usize = 3 << 30;
const CHECKPOINT_EVERY: u32 = 30;

pub fn bake(on_progress: ProgressCallback) -> rikka_physics::Result<Bake> {
    Bake::builder()
        .thread("rikka-animate-pour")
        .budget(CACHE_BUDGET)
        .checkpoint_every(CHECKPOINT_EVERY)
        .spawn(on_progress)
}

pub(crate) fn cache_of(progress: &Progress) -> LiquidCache {
    let frames =
        |from: u32, through: u32| frame_at_step(from)..frame_at_step(through.saturating_add(1));
    let ahead = progress
        .cached
        .first()
        .filter(|range| *range.start() == 0)
        .map_or(0, |range| range.end() + 1);
    LiquidCache {
        ready: progress
            .cached
            .iter()
            .map(|range| frames(*range.start(), *range.end()))
            .collect(),
        computing: (progress.working && ahead <= progress.target_step)
            .then(|| frames(ahead, progress.target_step)),
    }
}
