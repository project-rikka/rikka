use std::sync::Arc;
use std::time::{Duration, Instant};

use replay_ring::PictureId;
use rikka_capture::{BufferPool, Captured, Constraints, Frames, Screen};
use rikka_video::{Coded, Feed, Picture, PixelLayout};

use crate::clock;
use crate::engine::coder::Coder;
use crate::engine::gpu::Gpu;
use crate::engine::target::Target;
use crate::error::{Error, Result};
use crate::options::CaptureOptions;
use crate::store::{Entry, SharedStore};

const BUFFERS: usize = 3;

#[derive(Debug)]
pub(crate) enum Health {
    Alive,
    Ended,
    Broken(Error),
}

pub(crate) struct Subject {
    pool: Option<BufferPool>,
    coder: Option<Coder>,
    frames: Frames,
    target: Target,
    pictures: Option<PictureId>,
    requested_at: Option<Instant>,
    last_us: i64,
    keyframe_fresh: bool,
    health: Health,
}

impl Subject {
    pub(crate) fn start(screen: &mut Screen, target: Target, cursor: bool) -> Result<Subject> {
        let frames = screen.frames(&target.source, cursor)?;
        Ok(Subject {
            pool: None,
            coder: None,
            frames,
            target,
            pictures: None,
            requested_at: None,
            last_us: 0,
            keyframe_fresh: true,
            health: Health::Alive,
        })
    }

    pub(crate) fn subject(&self) -> &str {
        &self.target.subject
    }

    pub(crate) fn health(&self) -> &Health {
        &self.health
    }

    pub(crate) fn update_kind(&mut self, target: Target, store: &SharedStore) -> bool {
        if target.kind == self.target.kind {
            return false;
        }
        if let Some(pictures) = self.pictures {
            store.lock().set_kind(pictures, target.kind.clone());
        }
        self.target.kind = target.kind;
        true
    }

    pub(crate) fn wake_at(&self, interval: Duration, now: Instant) -> Option<Instant> {
        let pool = self.pool.as_ref()?;
        let asking =
            (!pool.is_pending()).then(|| self.requested_at.map_or(now, |at| at + interval));
        let refreshing = (!self.keyframe_fresh && pool.latest().is_some()).then(|| {
            let due = self.last_us + clock::micros(Coder::keyframes_every());
            now + Duration::from_micros((due - clock::now_us()).max(0) as u64)
        });
        asking.into_iter().chain(refreshing).min()
    }

    pub(crate) fn pump(
        &mut self,
        screen: &mut Screen,
        gpu: &mut Gpu,
        capture: &CaptureOptions,
        store: &SharedStore,
        now: Instant,
    ) -> bool {
        if !matches!(self.health, Health::Alive) {
            return false;
        }
        if self.frames.is_stopped() {
            self.health = Health::Ended;
            return false;
        }
        let constraints = self.frames.constraints();
        if constraints.is_empty() {
            return false;
        }
        let mut added = false;
        let stale = self
            .pool
            .as_ref()
            .is_none_or(|pool| pool.generation() != constraints.generation && !pool.is_pending());
        if stale {
            match self.prepare(screen, gpu, capture, store, &constraints) {
                Ok(opened) => added = opened,
                Err(reason) => {
                    self.health = Health::Broken(reason);
                    return false;
                }
            }
        }
        self.refresh_keyframe(store);
        let interval = Duration::from_secs_f64(1.0 / f64::from(capture.fps));
        if self.requested_at.is_none_or(|at| now >= at + interval) {
            self.ask(now);
        }
        added
    }

    pub(crate) fn collect_frames(&mut self, store: &SharedStore) {
        for captured in self.frames.collect(self.pool.as_mut()) {
            match captured {
                Captured::Ready { index, presented } => {
                    let at = presented.unwrap_or_else(clock::now);
                    self.encode(index, at, false, store);
                }
                Captured::Lost => {}
                Captured::Ended => self.health = Health::Ended,
            }
        }
    }

    pub(crate) fn end(&mut self, store: &SharedStore) {
        if let Some(pictures) = self.pictures.take() {
            store.lock().end(pictures);
        }
    }

    fn prepare(
        &mut self,
        screen: &mut Screen,
        gpu: &mut Gpu,
        capture: &CaptureOptions,
        store: &SharedStore,
        constraints: &Constraints,
    ) -> Result<bool> {
        self.pool = None;
        let size = (constraints.width, constraints.height);
        let device = constraints.dmabuf.as_ref().map(|offered| offered.device);
        let shared = device.and_then(|device| {
            let device = gpu.device_for(device)?;
            BufferPool::dmabuf(screen, device, constraints, BUFFERS)
                .inspect_err(|error| tracing::debug!("no shared capture buffers: {error}"))
                .ok()
        });
        let (canvas, feed) = match (&shared, self.target.canvas) {
            (Some(_), Some(canvas)) => (canvas, Feed::Dmabuf),
            (Some(_), None) => (even(size), Feed::Dmabuf),
            (None, _) => (even(size), Feed::Memory),
        };
        let kept = self
            .coder
            .take()
            .filter(|coder| coder.canvas == canvas && coder.feed == feed);
        let reopen = kept.is_none();
        let coder = match kept {
            Some(coder) => coder,
            None => self.open_coder(gpu, capture, store, (canvas, feed), device, size)?,
        };
        let coder = self.coder.insert(coder);
        let pool = match (coder.feed, shared) {
            (Feed::Dmabuf, Some(mut pool)) => {
                pool.bind(|spec| coder.video.import(spec))
                    .map_err(Error::Import)?;
                pool
            }
            _ => BufferPool::shm(screen, constraints, BUFFERS)?,
        };
        self.pool = Some(pool);
        Ok(reopen)
    }

    fn open_coder(
        &mut self,
        gpu: &mut Gpu,
        capture: &CaptureOptions,
        store: &SharedStore,
        (canvas, feed): ((u32, u32), Feed),
        device: Option<u64>,
        size: (u32, u32),
    ) -> Result<Coder> {
        let coder = match feed {
            Feed::Dmabuf => {
                Coder::open(gpu.any(), capture, canvas, Feed::Dmabuf).or_else(|reason| {
                    tracing::warn!(
                        "{}: GPU encoder rejected dmabuf input ({reason}); falling back to shm",
                        self.target.subject
                    );
                    if let Some(device) = device {
                        gpu.refuse(device);
                    }
                    Coder::open(gpu.any(), capture, even(size), Feed::Memory)
                })
            }
            Feed::Memory => Coder::open(gpu.any(), capture, canvas, Feed::Memory),
        }?;
        let stream = coder.video.stream()?;
        tracing::info!(
            "recording {} at {}x{} with {} ({:?})",
            self.target.subject,
            coder.canvas.0,
            coder.canvas.1,
            coder.video.name(),
            coder.feed,
        );
        let mut store = store.lock();
        if let Some(old) = self.pictures.take() {
            store.end(old);
        }
        let pictures = store.ring.open_pictures();
        store.push_entry(Entry {
            subject: self.target.subject.clone(),
            kind: self.target.kind.clone(),
            pictures,
            stream: Arc::new(stream),
        });
        drop(store);
        self.pictures = Some(pictures);
        self.keyframe_fresh = true;
        Ok(coder)
    }

    fn ask(&mut self, now: Instant) {
        let Some(pool) = &mut self.pool else {
            return;
        };
        match pool.capture(&mut self.frames) {
            Ok(true) => self.requested_at = Some(now),
            Ok(false) => {}
            Err(error) => self.health = Health::Broken(Error::Screen(error)),
        }
    }

    fn refresh_keyframe(&mut self, store: &SharedStore) {
        if self.keyframe_fresh {
            return;
        }
        let Some(pool) = &self.pool else {
            return;
        };
        let Some(latest) = pool.latest() else {
            return;
        };
        if clock::now_us() - self.last_us < clock::micros(Coder::keyframes_every()) {
            return;
        }
        if !pool.is_idle(latest) {
            return;
        }
        self.encode(latest, clock::now(), true, store);
        self.keyframe_fresh = true;
    }

    fn encode(&mut self, index: usize, at: Duration, keyframe: bool, store: &SharedStore) {
        let (Some(coder), Some(pictures), Some(pool)) =
            (self.coder.as_mut(), self.pictures, self.pool.as_ref())
        else {
            return;
        };
        if keyframe {
            coder.video.request_keyframe();
        }
        let coded = match (pool.imported(index), coder.feed) {
            (Some(imported), Feed::Dmabuf) => coder.video.encode_imported(imported, at),
            (_, Feed::Memory) => {
                let buffer = pool.buffer(index);
                let Some(pixels) = buffer.pixels() else {
                    return;
                };
                let picture = Picture {
                    bytes: pixels,
                    width: coder.canvas.0,
                    height: coder.canvas.1,
                    stride: buffer.stride() as usize,
                    layout: match buffer.format().is_bgr() {
                        true => PixelLayout::Bgra,
                        false => PixelLayout::Rgba,
                    },
                };
                coder.video.encode(picture, at)
            }
            (None, Feed::Dmabuf) => return,
        };
        match coded {
            Ok(coded) => {
                self.last_us = clock::micros(at);
                if let Some(last) = coded.last() {
                    self.keyframe_fresh = last.keyframe;
                }
                keep(store, pictures, coded);
            }
            Err(error) => {
                self.health = Health::Broken(Error::Encode(error));
            }
        }
    }
}

fn even((width, height): (u32, u32)) -> (u32, u32) {
    (width & !1, height & !1)
}

fn keep(store: &SharedStore, pictures: PictureId, coded: Vec<Coded>) {
    if coded.is_empty() {
        return;
    }
    let mut store = store.lock();
    for packet in coded {
        store.ring.push_picture(pictures, packet);
    }
}
