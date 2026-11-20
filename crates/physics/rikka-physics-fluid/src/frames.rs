use std::collections::VecDeque;

use glam::Vec3;
use rikka_gpu::Download;
use rikka_physics_gpu::words;

use crate::error::{Error, Result};

pub(crate) const STATS_WORDS: usize = 11;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Stats {
    pub(crate) time: f64,
    pub(crate) spawned: u64,
    pub(crate) live: u32,
    pub(crate) fastest: f32,
    pub(crate) entries: u32,
    pub(crate) foam: u32,
    pub(crate) colours: u32,
    pub(crate) bounds: Option<[Vec3; 2]>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Measured {
    pub(crate) latest: Stats,
    earlier: Stats,
}

impl Measured {
    pub(crate) fn basis(&self, in_flight: usize) -> Stats {
        match in_flight {
            0 => self.earlier,
            _ => self.latest,
        }
    }

    fn arrive(&mut self, stats: Stats) {
        self.earlier = self.latest;
        self.latest = stats;
    }
}

struct Submitted {
    stats: Download,
    time: f64,
    spawned: u64,
}

#[derive(Default)]
pub(crate) struct InFlight {
    frames: VecDeque<Submitted>,
}

impl InFlight {
    pub(crate) fn push(&mut self, stats: Download, time: f64, spawned: u64) {
        self.frames.push_back(Submitted {
            stats,
            time,
            spawned,
        });
    }

    pub(crate) fn len(&self) -> usize {
        self.frames.len()
    }

    pub(crate) fn settle(&mut self, measured: &mut Measured, keep: usize) -> Result<()> {
        while self.frames.len() > keep {
            let Some(frame) = self.frames.pop_front() else {
                break;
            };
            measured.arrive(decode(frame)?);
        }
        Ok(())
    }
}

fn decode(frame: Submitted) -> Result<Stats> {
    let words: Vec<u32> = words(frame.stats, 4 * STATS_WORDS as u64, Error::ShortReadback)?;
    let bounds = (words[0] > 0).then(|| {
        [
            -Vec3::new(
                unordered(words[7]),
                unordered(words[8]),
                unordered(words[9]),
            ),
            Vec3::new(
                unordered(words[4]),
                unordered(words[5]),
                unordered(words[6]),
            ),
        ]
    });
    Ok(Stats {
        time: frame.time,
        spawned: frame.spawned,
        live: words[0],
        fastest: f32::from_bits(words[1]),
        entries: words[2],
        foam: words[3],
        colours: words[10],
        bounds,
    })
}

fn unordered(key: u32) -> f32 {
    match key & 0x8000_0000 {
        0 => f32::from_bits(!key),
        _ => f32::from_bits(key & 0x7fff_ffff),
    }
}
