use std::collections::hash_map::Entry as MapEntry;
#[cfg(unix)]
use std::os::fd::AsFd;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(unix)]
use rikka_gpu::DmabufPlane;
use rikka_gpu::{ComputedFrame, Pixels, TextureFormat};
use rikka_image::{Budget, ColorScheme, Image, ImageLoader, LoadHandle, Reservation};
use rikka_picture::consume::{
    ComputedConsumer, Content, DamageLog, FeedConsumer, ImportedConsumer, announce_arrival,
};
use rikka_picture::{Changes, ComputedImage, Feed, ImportedBuffer, ImportedFrame, Lease, Pulse};
#[cfg(unix)]
use rikka_renderer::DmabufSpec;
use rikka_renderer::{TextureId, Uploads};
use rikka_types::Rect;
use rikka_ui::{ImageSource, SourceKey, TextureResolver, VectorSource};
use rustc_hash::FxHashMap;

use crate::resources::warn_video_unresolved;

const DEFAULT_OFF_SCREEN: usize = 64 * 1024 * 1024;

const DEFAULT_DECODE_BUDGET: usize = 128 * 1024 * 1024;

const UPLOAD_PER_FRAME: usize = 16 * 1024 * 1024;

const RING_KEPT: usize = 4;

const FAILED_TTL: Duration = Duration::from_secs(10);

const FAILED_SWEEP: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub struct ImageCache {
    loader: ImageLoader,
    entries: FxHashMap<SourceKey, Entry>,
    in_flight: Vec<SourceKey>,
    backlog: bool,
    tick: u64,
    off_screen: usize,
    failures: usize,
    swept: Instant,
    uploads: Option<Uploads>,
    repainted: bool,
    pulsed: Vec<(SourceKey, Pulse)>,
    scheme: ColorScheme,
}

#[derive(Debug)]
struct Entry {
    state: State,
    bytes: usize,
    last_used: u64,
    shown: bool,
    asked: ColorScheme,
    follows_scheme: bool,
    stale: Option<Uploaded>,
}

impl Entry {
    fn is_evictable(&self) -> bool {
        !self.shown && self.bytes > 0 && matches!(self.state, State::Ready(_))
    }

    fn is_forsaken(&self, key: &SourceKey) -> bool {
        !self.shown
            && match &self.state {
                State::Fed { feed, .. } => feed.is_abandoned(),
                State::Received { computed, .. } => computed.is_abandoned(),
                _ => key.is_unreachable(),
            }
    }
}

#[derive(Debug)]
enum State {
    Loading(LoadHandle),
    Pending {
        image: Image,
        reservation: Reservation,
    },
    Ready(Uploaded),
    Fed {
        feed: Feed,
        texture: Option<FedTexture>,
    },
    Received {
        computed: ComputedImage,
        texture: Option<Uploaded>,
    },
    Handed {
        imported: ImportedBuffer,
        textures: Vec<ImportedTexture>,
        current: Option<TextureId>,
        size: Option<(u32, u32)>,
        log: DamageLog,
    },
    Failed {
        at: Instant,
    },
}

#[derive(Debug)]
struct ImportedTexture {
    buffer: u64,
    texture: TextureId,
    lease: Option<Lease>,
}

#[derive(Debug)]
struct FedTexture {
    uploaded: Uploaded,
    video: bool,
}

#[derive(Debug)]
struct Uploaded {
    id: TextureId,
    width: u32,
    height: u32,
}

impl Uploaded {
    fn upload(uploads: &Uploads, image: Image) -> rikka_renderer::Result<Uploaded> {
        let (width, height) = image.dimensions();
        let pixels = image.into_rgba8();
        Uploaded::raw(uploads, width, height, pixels)
    }

    fn raw(
        uploads: &Uploads,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    ) -> rikka_renderer::Result<Uploaded> {
        let id = uploads.create(Pixels::rgba8(width, height, pixels))?;
        Ok(Uploaded { id, width, height })
    }

    fn id(&self) -> TextureId {
        self.id
    }

    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn destroy(self, uploads: &Uploads) {
        uploads.destroy(self.id);
    }
}

fn release_imported(uploads: &Uploads, imported: ImportedTexture) {
    uploads.destroy(imported.texture);
    if let Some(lease) = imported.lease {
        uploads.reclaimer().defer_drop(lease);
    }
}

#[cfg(unix)]
fn import(uploads: &Uploads, arrival: &ImportedFrame) -> rikka_renderer::Result<TextureId> {
    let planes: Vec<DmabufPlane<'_>> = arrival
        .planes
        .iter()
        .map(|plane| DmabufPlane {
            offset: plane.offset,
            stride: plane.stride,
            fd: plane.fd.as_fd(),
        })
        .collect();
    uploads.import_dmabuf(&DmabufSpec {
        width: arrival.width,
        height: arrival.height,
        fourcc: arrival.fourcc,
        modifier: arrival.modifier,
        planes: &planes,
    })
}

#[cfg(windows)]
fn import(_uploads: &Uploads, arrival: &ImportedFrame) -> rikka_renderer::Result<TextureId> {
    match arrival.planes.first() {
        Some(plane) => match *plane {},
        None => Err(rikka_renderer::Error::Invalid(
            "imported frame has no planes".into(),
        )),
    }
}

impl ImageCache {
    #[must_use]
    pub fn new() -> ImageCache {
        ImageCache::keeping(DEFAULT_OFF_SCREEN)
    }

    #[must_use]
    pub fn keeping(off_screen: usize) -> ImageCache {
        ImageCache {
            loader: ImageLoader::new()
                .with_budget(Budget::new(DEFAULT_DECODE_BUDGET))
                .notifying(announce_arrival),
            entries: FxHashMap::default(),
            in_flight: Vec::new(),
            backlog: false,
            tick: 0,
            off_screen,
            failures: 0,
            swept: Instant::now(),
            uploads: None,
            repainted: false,
            pulsed: Vec::new(),
            scheme: ColorScheme::default(),
        }
    }

    pub(crate) fn set_color_scheme(&mut self, scheme: ColorScheme) {
        self.scheme = scheme;
    }

    pub fn shown<'a>(&mut self, drawn: impl Iterator<Item = &'a SourceKey>) {
        for entry in self.entries.values_mut() {
            entry.shown = false;
        }
        let tick = self.tick;
        for key in drawn {
            let Some(entry) = self.entries.get_mut(key) else {
                continue;
            };
            entry.shown = true;
            entry.last_used = tick;
            if let State::Failed { at } = &mut entry.state {
                *at = Instant::now();
            }
        }
    }

    pub fn commit(
        &mut self,
        uploads: &impl AsRef<Uploads>,
        settled: &mut Vec<SourceKey>,
        updated: &mut Vec<TextureId>,
    ) {
        let uploads = uploads.as_ref();
        self.uploads = Some(uploads.clone());
        self.feed(uploads, settled, updated);
        self.receive(uploads, settled, updated);
        self.hand_over(uploads, settled, updated);
        self.advance(uploads, settled);
        self.release_imported(uploads);
        self.evict(uploads);
        self.sweep();
        self.tick += 1;
    }

    fn advance(&mut self, uploads: &Uploads, settled: &mut Vec<SourceKey>) {
        let mut room = UPLOAD_PER_FRAME;
        self.backlog = false;
        let mut flying = Vec::with_capacity(self.in_flight.len());
        let mut given_up = Vec::new();

        for key in std::mem::take(&mut self.in_flight) {
            let Some(entry) = self.entries.get_mut(&key) else {
                continue;
            };
            if !entry.shown {
                given_up.push(key);
                continue;
            }
            if let State::Loading(handle) = &entry.state {
                if !handle.is_ready() {
                    flying.push(key);
                    continue;
                }
                match handle.try_take() {
                    None => {
                        flying.push(key);
                        continue;
                    }
                    Some(Ok(image)) => {
                        entry.follows_scheme = handle.follows_color_scheme();
                        let reservation = self.loader.budget().reserve_now(pixel_bytes(&image));
                        entry.state = State::Pending { image, reservation };
                    }
                    Some(Err(error)) => {
                        tracing::warn!("failed to decode image: {error}");
                        entry.state = State::Failed { at: Instant::now() };
                        if let Some(stale) = entry.stale.take() {
                            stale.destroy(uploads);
                        }
                        self.failures += 1;
                        settled.push(key);
                        continue;
                    }
                }
            }
            if !matches!(entry.state, State::Pending { .. }) {
                continue;
            }
            if room == 0 {
                self.backlog = true;
                flying.push(key);
                continue;
            }
            let State::Pending { image, reservation } =
                std::mem::replace(&mut entry.state, State::Failed { at: Instant::now() })
            else {
                continue;
            };
            let bytes = pixel_bytes(&image);
            let uploaded = Uploaded::upload(uploads, image);
            drop(reservation);
            match uploaded {
                Ok(texture) => {
                    entry.state = State::Ready(texture);
                    entry.bytes = bytes;
                    room = room.saturating_sub(bytes);
                }
                Err(error) => {
                    tracing::warn!("failed to upload image: {error}");
                    self.failures += 1;
                }
            }
            if let Some(stale) = entry.stale.take() {
                stale.destroy(uploads);
            }
            settled.push(key);
        }

        self.in_flight = flying;
        for key in given_up {
            if let Some(entry) = self.entries.remove(&key) {
                self.forget(uploads, entry);
            }
        }
    }

    fn feed(
        &mut self,
        uploads: &Uploads,
        settled: &mut Vec<SourceKey>,
        updated: &mut Vec<TextureId>,
    ) {
        for (key, entry) in &mut self.entries {
            let State::Fed { feed, texture } = &mut entry.state else {
                continue;
            };
            if !feed.is_attached() {
                feed.attach(uploads.video_device());
            }
            let Some(arrival) = feed.take() else { continue };
            let size = (arrival.width, arrival.height);
            let video = matches!(arrival.content, Content::Video(_));
            let fitting = texture
                .as_ref()
                .filter(|kept| kept.uploaded.dimensions() == size && kept.video == video);
            if let Some(kept) = fitting {
                let id = kept.uploaded.id();
                let shown = match arrival.content {
                    Content::Video(frame) => unsafe { uploads.submit_video_frame(id, frame) },
                    Content::Pixels(pixels) => uploads.update(id, TextureFormat::Rgba8, pixels),
                };
                match shown {
                    Ok(()) => updated.push(id),
                    Err(error) => tracing::warn!("failed to upload feed frame: {error}"),
                }
                continue;
            }
            if let Some(stale) = texture.take() {
                stale.uploaded.destroy(uploads);
            }
            let made = match arrival.content {
                Content::Video(frame) => uploads
                    .blank(size.0, size.1)
                    .and_then(|id| unsafe { uploads.submit_video_frame(id, frame) }.map(|()| id)),
                Content::Pixels(pixels) => uploads.create(Pixels::rgba8(size.0, size.1, pixels)),
            };
            match made {
                Ok(id) => {
                    entry.bytes = size.0 as usize * size.1 as usize * 4;
                    *texture = Some(FedTexture {
                        uploaded: Uploaded {
                            id,
                            width: size.0,
                            height: size.1,
                        },
                        video,
                    });
                    settled.push(key.clone());
                }
                Err(error) => {
                    tracing::warn!("failed to upload initial feed frame: {error}");
                    entry.bytes = 0;
                }
            }
        }
    }

    fn receive(
        &mut self,
        uploads: &Uploads,
        settled: &mut Vec<SourceKey>,
        updated: &mut Vec<TextureId>,
    ) {
        for (key, entry) in &mut self.entries {
            let State::Received { computed, texture } = &mut entry.state else {
                continue;
            };
            let took = take_frame(uploads, computed, texture, &mut entry.bytes);
            if took.made {
                settled.push(key.clone());
            }
            updated.extend(took.refreshed);
        }
    }

    fn hand_over(
        &mut self,
        uploads: &Uploads,
        settled: &mut Vec<SourceKey>,
        updated: &mut Vec<TextureId>,
    ) {
        let reclaimer = uploads.reclaimer();
        let mut refused = Vec::new();
        let mut repainted = false;
        for (key, entry) in &mut self.entries {
            let State::Handed {
                imported,
                textures,
                current,
                size,
                log,
            } = &mut entry.state
            else {
                continue;
            };
            let Some(mut arrival) = imported.take() else {
                continue;
            };
            if let Some(pulse) = arrival.pulse.take() {
                self.pulsed.push((key.clone(), pulse));
            }
            let lease = arrival.lease.take();
            let shaped = (arrival.width, arrival.height);
            let damage = arrival
                .damage
                .take()
                .filter(|_| current.is_some() && *size == Some(shaped))
                .map(|areas| fractions(&areas, shaped));
            *size = Some(shaped);
            let partial = log.record(damage);
            repainted |= partial;

            if let Some(superseded) = textures.iter_mut().find(|imported| {
                Some(imported.texture) == *current && imported.buffer != arrival.buffer
            }) && let Some(lease) = superseded.lease.take()
            {
                reclaimer.defer_drop(lease);
            }

            if let Some(imported) = textures
                .iter_mut()
                .find(|imported| imported.buffer == arrival.buffer)
            {
                uploads.notify_committed(imported.texture);
                if !partial {
                    updated.push(imported.texture);
                }
                *current = Some(imported.texture);
                if let Some(stale) = std::mem::replace(&mut imported.lease, lease) {
                    reclaimer.defer_drop(stale);
                }
                continue;
            }

            let made = import(uploads, &arrival);
            match made {
                Ok(texture) => {
                    textures.push(ImportedTexture {
                        buffer: arrival.buffer,
                        texture,
                        lease,
                    });
                    *current = Some(texture);
                    settled.push(key.clone());
                    if !partial {
                        updated.push(texture);
                    }
                }
                Err(error) => {
                    tracing::warn!("failed to import buffer: {error}");
                    refused.push(key.clone());
                    continue;
                }
            }
            while textures.len() > RING_KEPT {
                release_imported(uploads, textures.remove(0));
            }
        }
        self.repainted |= repainted;

        for key in refused {
            let Some(entry) = self.entries.get_mut(&key) else {
                continue;
            };
            if let State::Handed { textures, .. } = &mut entry.state {
                for imported in std::mem::take(textures) {
                    release_imported(uploads, imported);
                }
            }
            entry.state = State::Failed { at: Instant::now() };
            self.failures += 1;
            settled.push(key);
        }
    }

    fn release_imported(&mut self, uploads: &Uploads) {
        let evicted: Vec<SourceKey> = self
            .entries
            .iter()
            .filter(|(_, entry)| !entry.shown && matches!(entry.state, State::Handed { .. }))
            .map(|(key, _)| key.clone())
            .collect();
        for key in evicted {
            if let Some(entry) = self.entries.remove(&key) {
                self.forget(uploads, entry);
            }
        }
    }

    pub(crate) fn has_backlog(&self) -> bool {
        self.backlog
    }

    #[must_use]
    pub fn is_loading(&self) -> bool {
        !self.in_flight.is_empty()
    }

    #[must_use]
    pub fn has_work(&self) -> bool {
        !self.in_flight.is_empty()
            || self.entries.values().any(|entry| {
                matches!(
                    entry.state,
                    State::Fed { .. } | State::Received { .. } | State::Handed { .. }
                )
            })
            || self.owes_eviction()
            || self.sweep_due()
    }

    pub fn release(&mut self, uploads: &impl AsRef<Uploads>) {
        let uploads = uploads.as_ref();
        self.uploads = None;
        self.in_flight.clear();
        for (_, entry) in std::mem::take(&mut self.entries) {
            self.forget(uploads, entry);
        }
    }

    pub fn take_repainted(&mut self) -> bool {
        std::mem::take(&mut self.repainted)
    }

    pub(crate) fn take_pulsed(&mut self) -> Vec<(SourceKey, Pulse)> {
        std::mem::take(&mut self.pulsed)
    }

    fn changes(&self, source: &ImageSource, since: u64) -> Changes {
        match self.entries.get(&source.key()).map(|entry| &entry.state) {
            Some(State::Handed { log, .. }) => log.since(since),
            _ => Changes::UNKNOWN,
        }
    }

    pub fn resolver(&mut self) -> CacheResolver<'_> {
        CacheResolver { cache: self }
    }

    fn resolve(&mut self, source: &ImageSource) -> Option<TextureId> {
        if let ImageSource::Texture { id, .. } = source {
            return Some(*id);
        }
        if let ImageSource::Video(_) = source {
            warn_video_unresolved();
            return None;
        }
        let ImageCache {
            loader,
            entries,
            in_flight,
            tick,
            failures,
            uploads,
            scheme,
            ..
        } = self;
        let entry = match entries.entry(source.key()) {
            MapEntry::Occupied(occupied) => occupied.into_mut(),
            MapEntry::Vacant(vacant) => {
                let mut state = start(loader, source, *scheme);
                let mut bytes = 0;
                if let (State::Received { computed, texture }, Some(uploads)) =
                    (&mut state, uploads.as_ref())
                {
                    take_frame(uploads, computed, texture, &mut bytes);
                }
                match state {
                    State::Loading(_) | State::Pending { .. } => {
                        in_flight.push(vacant.key().clone());
                    }
                    State::Failed { .. } => *failures += 1,
                    State::Ready(_)
                    | State::Fed { .. }
                    | State::Received { .. }
                    | State::Handed { .. } => {}
                }
                vacant.insert(Entry {
                    state,
                    bytes,
                    last_used: *tick,
                    shown: true,
                    asked: *scheme,
                    follows_scheme: false,
                    stale: None,
                })
            }
        };
        entry.last_used = *tick;
        entry.shown = true;
        if entry.follows_scheme && entry.asked != *scheme && matches!(entry.state, State::Ready(_))
        {
            if let State::Ready(texture) =
                std::mem::replace(&mut entry.state, start(loader, source, *scheme))
            {
                entry.stale = Some(texture);
            }
            entry.asked = *scheme;
            in_flight.push(source.key());
        }
        match &mut entry.state {
            State::Ready(texture) => Some(texture.id()),
            State::Fed { texture, .. } => texture.as_ref().map(|kept| kept.uploaded.id()),
            State::Received { texture, .. } => texture.as_ref().map(Uploaded::id),
            State::Handed { current, .. } => *current,
            State::Failed { at } => {
                *at = Instant::now();
                None
            }
            State::Loading(_) | State::Pending { .. } => entry.stale.as_ref().map(Uploaded::id),
        }
    }

    fn evict(&mut self, uploads: &Uploads) {
        let forsaken: Vec<SourceKey> = self
            .entries
            .iter()
            .filter(|(key, entry)| entry.is_forsaken(key))
            .map(|(key, _)| key.clone())
            .collect();
        for key in forsaken {
            if let Some(entry) = self.entries.remove(&key) {
                self.forget(uploads, entry);
            }
        }

        let mut imported = self.off_screen_bytes();
        if imported <= self.off_screen {
            return;
        }
        let mut candidates: Vec<(u64, SourceKey)> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.is_evictable())
            .map(|(key, entry)| (entry.last_used, key.clone()))
            .collect();
        candidates.sort_unstable_by_key(|(last_used, _)| *last_used);
        for (_, key) in candidates {
            if imported <= self.off_screen {
                break;
            }
            if let Some(entry) = self.entries.remove(&key) {
                imported -= entry.bytes;
                self.forget(uploads, entry);
            }
        }
    }

    fn off_screen_bytes(&self) -> usize {
        self.entries
            .values()
            .filter(|entry| entry.is_evictable())
            .map(|entry| entry.bytes)
            .sum()
    }

    fn owes_eviction(&self) -> bool {
        let mut imported = 0;
        for (key, entry) in &self.entries {
            if entry.is_forsaken(key) {
                return true;
            }
            if entry.is_evictable() {
                imported += entry.bytes;
            }
        }
        imported > self.off_screen
    }

    fn forget(&mut self, uploads: &Uploads, entry: Entry) {
        if let Some(stale) = entry.stale {
            stale.destroy(uploads);
        }
        match entry.state {
            State::Ready(texture) => texture.destroy(uploads),
            State::Fed { texture, .. } => {
                if let Some(kept) = texture {
                    kept.uploaded.destroy(uploads);
                }
            }
            State::Received { texture, .. } => {
                if let Some(texture) = texture {
                    texture.destroy(uploads);
                }
            }
            State::Handed { textures, .. } => {
                for imported in textures {
                    release_imported(uploads, imported);
                }
            }
            State::Failed { .. } => self.failures -= 1,
            State::Loading(_) | State::Pending { .. } => {}
        }
    }

    fn sweep(&mut self) {
        if !self.sweep_due() {
            return;
        }
        self.swept = Instant::now();
        let mut failures = self.failures;
        self.entries.retain(|_, entry| match &entry.state {
            State::Failed { at } if !entry.shown && at.elapsed() >= FAILED_TTL => {
                failures -= 1;
                false
            }
            _ => true,
        });
        self.failures = failures;
    }

    fn sweep_due(&self) -> bool {
        self.failures > 0 && self.swept.elapsed() >= FAILED_SWEEP
    }
}

impl Default for ImageCache {
    fn default() -> ImageCache {
        ImageCache::new()
    }
}

#[derive(Debug)]
pub struct CacheResolver<'a> {
    cache: &'a mut ImageCache,
}

impl TextureResolver for CacheResolver<'_> {
    fn resolve(&mut self, source: &ImageSource) -> Option<TextureId> {
        self.cache.resolve(source)
    }

    fn changes(&mut self, source: &ImageSource, since: u64) -> Changes {
        self.cache.changes(source, since)
    }
}

#[derive(Default)]
struct ComputedOutcome {
    made: bool,
    refreshed: Option<TextureId>,
}

fn take_frame(
    uploads: &Uploads,
    computed: &ComputedImage,
    texture: &mut Option<Uploaded>,
    bytes: &mut usize,
) -> ComputedOutcome {
    let mut took = ComputedOutcome::default();
    let has_texture = texture.is_some();
    let upload = |frame: ComputedFrame| took = upload_frame(uploads, &frame, texture, bytes);
    match has_texture {
        true => computed.upload_when_ready(upload),
        false => computed.upload_now(upload),
    }
    took
}

fn upload_frame(
    uploads: &Uploads,
    frame: &ComputedFrame,
    texture: &mut Option<Uploaded>,
    bytes: &mut usize,
) -> ComputedOutcome {
    let mut took = ComputedOutcome::default();
    let (width, height) = (frame.width(), frame.height());
    let shaped = texture
        .as_ref()
        .is_some_and(|uploaded| uploaded.dimensions() == (width, height));
    if !shaped {
        if let Some(stale) = texture.take() {
            stale.destroy(uploads);
        }
        match uploads.create_compute_target(width, height) {
            Ok(id) => {
                *bytes = width as usize * height as usize * 4;
                took.made = true;
                *texture = Some(Uploaded { id, width, height });
            }
            Err(error) => {
                tracing::warn!("failed to allocate texture for computed image: {error}");
                *bytes = 0;
                return took;
            }
        }
    }
    let Some(target) = texture.as_ref() else {
        return took;
    };
    for (at, copy) in frame.parts() {
        if let Err(error) = uploads.copy_computed(target.id(), *at, copy.clone()) {
            tracing::warn!("failed to present computed image: {error}");
        }
    }
    if shaped {
        took.refreshed = Some(target.id());
    }
    took
}

fn pixel_bytes(image: &Image) -> usize {
    usize::try_from(image.pixel_count()).unwrap_or(usize::MAX) * 4
}

fn start(loader: &ImageLoader, source: &ImageSource, scheme: ColorScheme) -> State {
    match source {
        ImageSource::Path(path) => State::Loading(loader.load(path.to_path_buf())),
        ImageSource::Bytes(bytes) => State::Loading(loader.load(Arc::clone(bytes))),
        ImageSource::Rgba {
            width,
            height,
            pixels,
        } => match Image::from_rgba8(*width, *height, pixels.to_vec()) {
            Ok(image) => {
                let reservation = loader.budget().reserve_now(pixel_bytes(&image));
                State::Pending { image, reservation }
            }
            Err(error) => {
                tracing::warn!("failed to upload {width}x{height} RGBA image: {error}");
                State::Failed { at: Instant::now() }
            }
        },
        ImageSource::Vector(vector) => match (vector.size(), vector.document()) {
            (Some((width, height)), VectorSource::Path(path)) => State::Loading(
                loader
                    .read(path.to_path_buf())
                    .vector(width, height)
                    .color_scheme(scheme)
                    .submit(),
            ),
            (Some((width, height)), VectorSource::Bytes(bytes)) => State::Loading(
                loader
                    .read(Arc::clone(bytes))
                    .vector(width, height)
                    .color_scheme(scheme)
                    .submit(),
            ),
            (None, _) => {
                tracing::warn!("vector image resolved before its size is known");
                State::Failed { at: Instant::now() }
            }
        },
        ImageSource::Computed(computed) => State::Received {
            computed: computed.clone(),
            texture: None,
        },
        ImageSource::Feed(feed) => State::Fed {
            feed: feed.clone(),
            texture: None,
        },
        ImageSource::Imported(imported) => State::Handed {
            imported: imported.clone(),
            textures: Vec::new(),
            current: None,
            size: None,
            log: DamageLog::default(),
        },
        ImageSource::Texture { .. } | ImageSource::Video(_) => State::Failed { at: Instant::now() },
    }
}

fn fractions(areas: &[Rect], (width, height): (u32, u32)) -> Vec<Rect> {
    let (width, height) = (width as f32, height as f32);
    areas
        .iter()
        .map(|area| {
            Rect::new(
                (area.x() - 1.0) / width,
                (area.y() - 1.0) / height,
                (area.width() + 2.0) / width,
                (area.height() + 2.0) / height,
            )
        })
        .collect()
}
