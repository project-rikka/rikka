use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::thread::JoinHandle;

use rikka_sync::{Condvar, Mutex};
use rikka_text::{CurveBlob, GlyphRaster};

use crate::text::atlas::{AtlasKey, RasterizedGlyph, SharedAtlas, draw};
use rikka_gpu::core::frame::FrameTimeline;

const MAX_WORKERS: usize = 3;

const SHARED_FROM: usize = 12;

const AHEAD_BATCH: usize = 8;

const MAX_AHEAD: usize = 512;

pub(crate) struct GlyphWorkers {
    queue: Arc<Queue>,
    atlas: SharedAtlas,
    timeline: Arc<FrameTimeline>,
    threads: Mutex<Vec<JoinHandle<()>>>,
    spawned: AtomicUsize,
}

#[derive(Default)]
struct Queue {
    jobs: Mutex<Jobs>,
    wake: Condvar,
}

#[derive(Default)]
struct Jobs {
    urgent: VecDeque<Urgent>,
    ahead: VecDeque<AtlasKey>,
    closed: bool,
}

struct Urgent {
    keys: Vec<AtlasKey>,
    reply: Sender<Vec<RasterizedGlyph>>,
}

enum Job {
    Urgent(Urgent),
    Ahead(Vec<AtlasKey>),
}

impl GlyphWorkers {
    pub(crate) fn new(atlas: SharedAtlas, timeline: Arc<FrameTimeline>) -> Self {
        Self {
            queue: Arc::default(),
            atlas,
            timeline,
            threads: Mutex::new(Vec::new()),
            spawned: AtomicUsize::new(0),
        }
    }

    pub(crate) fn available(&self) -> bool {
        self.spawned() > 0
    }

    pub(crate) fn draw_ahead(&self, keys: impl IntoIterator<Item = AtlasKey>) {
        let mut dropped = Vec::new();
        {
            let mut jobs = self.queue.jobs.lock();
            let before = jobs.ahead.len();
            jobs.ahead.extend(keys);
            if jobs.ahead.len() == before {
                return;
            }
            if jobs.ahead.len() > MAX_AHEAD {
                let drop_count = jobs.ahead.len() - MAX_AHEAD;
                dropped.extend(jobs.ahead.drain(..drop_count));
            }
        }
        if !dropped.is_empty() {
            self.atlas.lock().cancel_warming(&dropped);
        }
        self.queue.wake.notify_all();
    }

    pub(crate) fn draw_all(
        &self,
        raster: &mut GlyphRaster,
        keys: Vec<AtlasKey>,
    ) -> Vec<RasterizedGlyph> {
        let mut blob = CurveBlob::new();
        let helpers = match keys.len() >= SHARED_FROM {
            true => self.spawned(),
            false => 0,
        };
        if helpers == 0 {
            return keys
                .into_iter()
                .map(|key| draw(raster, &mut blob, key))
                .collect();
        }
        let share = keys.len().div_ceil(helpers + 1);
        let mut chunks = keys.chunks(share);
        let own = chunks.next().unwrap_or_default();
        let (reply, replies) = channel();
        let sent: Vec<&[AtlasKey]> = chunks.collect();
        {
            let mut jobs = self.queue.jobs.lock();
            for chunk in &sent {
                jobs.urgent.push_back(Urgent {
                    keys: chunk.to_vec(),
                    reply: reply.clone(),
                });
            }
        }
        drop(reply);
        self.queue.wake.notify_all();
        let mut drawn: Vec<RasterizedGlyph> = own
            .iter()
            .map(|&key| draw(raster, &mut blob, key))
            .collect();
        let mut answered = 0;
        while let Ok(part) = replies.recv() {
            answered += part.len();
            drawn.extend(part);
        }
        if answered < keys.len() - own.len() {
            let have: rustc_hash::FxHashSet<AtlasKey> =
                drawn.iter().map(RasterizedGlyph::key).collect();
            for &key in sent.iter().flat_map(|chunk| chunk.iter()) {
                if !have.contains(&key) {
                    drawn.push(draw(raster, &mut blob, key));
                }
            }
        }
        drawn
    }

    fn spawned(&self) -> usize {
        let count = self.spawned.load(Ordering::Acquire);
        if count > 0 {
            return count;
        }
        let mut threads = self.threads.lock();
        if threads.is_empty() {
            let count = std::thread::available_parallelism()
                .map_or(1, |cores| cores.get() / 2)
                .clamp(1, MAX_WORKERS);
            for _ in 0..count {
                let queue = Arc::clone(&self.queue);
                let atlas = Arc::clone(&self.atlas);
                let timeline = Arc::clone(&self.timeline);
                let spawned = std::thread::Builder::new()
                    .name(String::from("rikka-glyphs"))
                    .spawn(move || work(&queue, &atlas, &timeline));
                match spawned {
                    Ok(thread) => threads.push(thread),
                    Err(error) => {
                        tracing::warn!("failed to spawn glyph worker: {error}");
                        break;
                    }
                }
            }
            self.spawned.store(threads.len(), Ordering::Release);
        }
        threads.len()
    }
}

impl Drop for GlyphWorkers {
    fn drop(&mut self) {
        self.queue.jobs.lock().closed = true;
        self.queue.wake.notify_all();
        for thread in self.threads.get_mut().drain(..) {
            let _ = thread.join();
        }
    }
}

fn work(queue: &Queue, atlas: &SharedAtlas, timeline: &FrameTimeline) {
    let mut raster = GlyphRaster::new();
    let mut blob = CurveBlob::new();
    while let Some(job) = next(queue) {
        match job {
            Job::Urgent(urgent) => {
                let drawn = urgent
                    .keys
                    .into_iter()
                    .map(|key| draw(&mut raster, &mut blob, key))
                    .collect();
                let _ = urgent.reply.send(drawn);
            }
            Job::Ahead(mut keys) => {
                {
                    let mut atlas = atlas.lock();
                    keys.retain(|key| atlas.is_wanted(key));
                }
                if keys.is_empty() {
                    continue;
                }
                let drawn: Vec<RasterizedGlyph> = keys
                    .into_iter()
                    .map(|key| draw(&mut raster, &mut blob, key))
                    .collect();
                let completed = timeline.completed();
                let mut atlas = atlas.lock();
                for drawn in drawn {
                    atlas.warmed(drawn, completed);
                }
            }
        }
    }
}

fn next(queue: &Queue) -> Option<Job> {
    let mut jobs = queue.jobs.lock();
    loop {
        if jobs.closed {
            return None;
        }
        if let Some(urgent) = jobs.urgent.pop_front() {
            return Some(Job::Urgent(urgent));
        }
        if !jobs.ahead.is_empty() {
            let take = jobs.ahead.len().min(AHEAD_BATCH);
            return Some(Job::Ahead(jobs.ahead.drain(..take).collect()));
        }
        jobs = queue.wake.wait(jobs);
    }
}
