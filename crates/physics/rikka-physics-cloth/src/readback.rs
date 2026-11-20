use rikka_sync::Mutex;
use std::collections::VecDeque;

use glam::Vec3;
use rikka_gpu::Download;
use rikka_physics_gpu::words;

use crate::cloth::Cloth;
use crate::error::{Error, Result};
use crate::figure::FigureId;

pub const READBACK_LAG: u32 = 4;
const LAG: u64 = READBACK_LAG as u64;
pub(crate) const STATS_WORDS: u64 = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    pub position: Vec3,
    pub velocity: Vec3,
}

struct PendingStats {
    step: u64,
    stats: Download,
    figures: Vec<(FigureId, u32)>,
}

impl PendingStats {
    fn movement(self) -> Result<Movement> {
        let words: Vec<u32> = words(
            self.stats,
            self.figures.len() as u64 * STATS_WORDS * 4,
            Error::ShortReadback,
        )?;
        Ok(self
            .figures
            .iter()
            .zip(words.as_chunks::<{ STATS_WORDS as usize }>().0)
            .map(|(&(_, particles), &[speed, jitter, moving])| Movement {
                speed: f32::from_bits(speed),
                jitter: f32::from_bits(jitter),
                moving: moving as f32 / particles.max(1) as f32,
            })
            .fold(Movement::default(), Movement::max))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Movement {
    pub(crate) speed: f32,
    pub(crate) jitter: f32,
    pub(crate) moving: f32,
}

impl Movement {
    fn max(self, other: Movement) -> Movement {
        Movement {
            speed: self.speed.max(other.speed),
            jitter: self.jitter.max(other.jitter),
            moving: self.moving.max(other.moving),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct StepStats {
    step: u64,
    movement: Movement,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WaitMode {
    Block,
    Ready,
}

#[derive(Default)]
struct StatsQueue {
    frames: VecDeque<PendingStats>,
    measured: VecDeque<StepStats>,
    reported: Movement,
    horizon: Option<u64>,
    failed: Option<Error>,
}

impl StatsQueue {
    fn settle_through(&mut self, step: u64, waiting: WaitMode) -> Result<()> {
        while let Some(frame) = self.frames.front() {
            if frame.step > step || (waiting == WaitMode::Ready && !frame.stats.is_ready()) {
                break;
            }
            let Some(frame) = self.frames.pop_front() else {
                break;
            };
            let step = frame.step;
            let movement = frame.movement()?;
            self.reported = movement;
            self.measured.push_back(StepStats { step, movement });
        }
        Ok(())
    }

    fn settle(&mut self) -> Result<()> {
        self.settle_through(u64::MAX, WaitMode::Block)
    }

    fn take_failure(&mut self) -> Result<()> {
        match self.failed.take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn observe(&mut self, step: u64, waiting: WaitMode) -> Result<Option<u64>> {
        self.take_failure()?;
        let Some(observed) = step.checked_sub(LAG) else {
            return Ok(None);
        };
        self.horizon = Some(observed);
        self.settle_through(observed, waiting)?;
        while self
            .measured
            .front()
            .is_some_and(|measured| measured.step < observed)
        {
            self.measured.pop_front();
        }
        Ok(Some(observed))
    }

    fn reported(&mut self) -> Movement {
        if let Some(horizon) = self.horizon
            && let Err(error) = self.settle_through(horizon, WaitMode::Block)
        {
            self.failed.get_or_insert(error);
        }
        self.reported
    }
}

#[derive(Default)]
pub(crate) struct StatsReadback {
    queue: Mutex<StatsQueue>,
}

impl StatsReadback {
    fn queue(&mut self) -> &mut StatsQueue {
        self.queue.get_mut()
    }

    pub(crate) fn push(&mut self, step: u64, stats: Download, figures: Vec<(FigureId, u32)>) {
        self.queue().frames.push_back(PendingStats {
            step,
            stats,
            figures,
        });
    }

    pub(crate) fn lagged(&mut self, step: u64) -> Result<Movement> {
        let queue = self.queue();
        let Some(observed) = queue.observe(step, WaitMode::Block)? else {
            return Ok(Movement::default());
        };
        Ok(queue
            .measured
            .front()
            .filter(|measured| measured.step == observed)
            .map_or(Movement::default(), |measured| measured.movement))
    }

    pub(crate) fn drain(&mut self, step: u64) -> Result<()> {
        self.queue().observe(step, WaitMode::Ready).map(drop)
    }

    pub(crate) fn history(&mut self, steps: u64) -> Result<Vec<StepStats>> {
        let queue = self.queue();
        queue.take_failure()?;
        queue.settle()?;
        let since = steps.saturating_sub(LAG);
        Ok(queue
            .measured
            .iter()
            .copied()
            .filter(|measured| measured.step >= since && measured.step < steps)
            .collect())
    }

    pub(crate) fn resume(&mut self, history: &[StepStats]) -> Result<()> {
        let queue = self.queue();
        queue.take_failure()?;
        queue.settle()?;
        queue.measured = history.iter().copied().collect();
        queue.horizon = None;
        Ok(())
    }

    fn reported(&self) -> Movement {
        self.queue.lock().reported()
    }
}

impl Cloth {
    #[must_use]
    pub fn motion(&self) -> f32 {
        self.stats_readback.reported().speed
    }

    pub fn read_particles(&mut self, figure: FigureId) -> Result<Vec<Particle>> {
        let figure_state = self.figure(figure)?;
        let count = u64::from(figure_state.count());
        let mut batch = self.compute.batch();
        let download = batch.download_range(&figure_state.state, 0, 2 * count * 16);
        batch.submit()?;
        let floats: Vec<[f32; 4]> = words(download, 2 * count * 16, Error::ShortReadback)?;
        let (positions, velocities) = floats.split_at(count as usize);
        Ok(figure_state
            .topology
            .gpu
            .iter()
            .flatten()
            .map(|&index| Particle {
                position: Vec3::from_slice(&positions[index as usize][..3]),
                velocity: Vec3::from_slice(&velocities[index as usize][..3]),
            })
            .collect())
    }

    pub fn read_positions(&self, figures: &[FigureId]) -> Result<Vec<Vec<Vec3>>> {
        let mut batch = self.compute.batch();
        let mut downloads = Vec::with_capacity(figures.len());
        for &figure in figures {
            let figure_state = self.figure(figure)?;
            let bytes = u64::from(figure_state.count()) * 16;
            downloads.push((
                figure_state,
                batch.download_range(&figure_state.state, 0, bytes),
                bytes,
            ));
        }
        batch.submit()?;
        downloads
            .into_iter()
            .map(|(figure_state, download, bytes)| {
                let positions: Vec<[f32; 4]> = words(download, bytes, Error::ShortReadback)?;
                Ok(figure_state
                    .topology
                    .gpu
                    .iter()
                    .flatten()
                    .map(|&index| Vec3::from_slice(&positions[index as usize][..3]))
                    .collect())
            })
            .collect()
    }
}
