use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cut_project::MediaId;
use rikka_image_edit::{Imaging, Picture};
use rikka_video::FrameReader;

use cut_project::Alignment;

const IDLE: Duration = Duration::from_secs(3);

pub(crate) type Listener = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone, Debug)]
pub(crate) enum FrameState {
    Exact(Picture),
    Near(Picture),
    Waiting,
    Offline,
}

impl FrameState {
    pub(crate) fn picture(&self) -> Option<&Picture> {
        match self {
            FrameState::Exact(picture) | FrameState::Near(picture) => Some(picture),
            FrameState::Waiting | FrameState::Offline => None,
        }
    }

    pub(crate) fn is_final(&self) -> bool {
        matches!(self, FrameState::Exact(_) | FrameState::Offline)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FrameRequest<'a> {
    pub(crate) alignment: Alignment,
    pub(crate) path: &'a Path,
    pub(crate) at: Duration,
    pub(crate) ahead: Duration,
    pub(crate) fit: Option<(u32, u32)>,
}

struct FootageStream {
    alignment: Alignment,
    path: PathBuf,
    reader: Option<FrameReader<Picture>>,
    fit: Option<Option<(u32, u32)>>,
    requested_fit: Option<Option<(u32, u32)>>,
    last: Option<Picture>,
    used: Instant,
    in_use: bool,
}

impl FootageStream {
    fn open(
        imaging: &Imaging,
        frame_request: &FrameRequest<'_>,
        wake: Option<&Listener>,
        now: Instant,
    ) -> FootageStream {
        let uploading = imaging.clone();
        let reader = FrameReader::open(frame_request.path, move |frame| {
            uploading
                .upload_rgba8(frame.width, frame.height, frame.rgba)
                .inspect_err(|error| tracing::warn!(%error, "failed to upload video frame"))
                .ok()
        })
        .inspect_err(|error| {
            tracing::warn!(%error, path = %frame_request.path.display(), "cannot open footage");
        })
        .ok();
        if let (Some(reader), Some(wake)) = (&reader, wake) {
            let wake = Arc::clone(wake);
            reader.on_ready(move || wake());
        }
        FootageStream {
            alignment: frame_request.alignment,
            path: frame_request.path.to_path_buf(),
            reader,
            fit: None,
            requested_fit: None,
            last: None,
            used: now,
            in_use: false,
        }
    }

    fn is_offline(&self) -> bool {
        self.reader
            .as_ref()
            .is_none_or(|reader| reader.failure().is_some())
    }

    fn reads(&self, path: &Path) -> bool {
        self.path == path
    }

    fn claim(&mut self, frame_request: &FrameRequest<'_>, now: Instant) {
        let fit = match (self.in_use, self.requested_fit) {
            (true, Some(requested)) => larger(requested, frame_request.fit),
            _ => frame_request.fit,
        };
        self.alignment = frame_request.alignment;
        self.in_use = true;
        self.used = now;
        self.requested_fit = Some(fit);
        let Some(reader) = &self.reader else {
            return;
        };
        if self.fit != Some(fit) {
            reader.fit(fit);
            self.fit = Some(fit);
        }
        reader.want(frame_request.at, frame_request.ahead);
    }

    fn fetch(&mut self, at: Duration) -> FrameState {
        if self.is_offline() {
            return FrameState::Offline;
        }
        let frame = self.reader.as_ref().and_then(|reader| reader.frame(at));
        match (frame, &self.last) {
            (Some((_, picture)), _) => {
                self.last = Some(picture.clone());
                FrameState::Exact(picture)
            }
            (None, Some(last)) => FrameState::Near(last.clone()),
            (None, None) => FrameState::Waiting,
        }
    }
}

fn larger(one: Option<(u32, u32)>, other: Option<(u32, u32)>) -> Option<(u32, u32)> {
    match (one, other) {
        (Some(one), Some(other)) => Some((one.0.max(other.0), one.1.max(other.1))),
        _ => None,
    }
}

pub(crate) struct Supply {
    imaging: Imaging,
    streams: Vec<FootageStream>,
    wake: Option<Listener>,
    round: u64,
}

impl Supply {
    pub(crate) fn new(imaging: Imaging) -> Supply {
        Supply {
            imaging,
            streams: Vec::new(),
            wake: None,
            round: 0,
        }
    }

    pub(crate) fn on_ready(&mut self, wake: Listener) {
        for reader in self
            .streams
            .iter()
            .filter_map(|stream| stream.reader.as_ref())
        {
            let wake = Arc::clone(&wake);
            reader.on_ready(move || wake());
        }
        self.wake = Some(wake);
    }

    pub(crate) fn begin(&mut self, round: u64) {
        if self.round != round {
            self.round = round;
            self.streams.retain(|stream| !stream.is_offline());
        }
        for stream in &mut self.streams {
            stream.in_use = false;
        }
    }

    pub(crate) fn fetch(&mut self, frame_request: &FrameRequest<'_>, now: Instant) -> FrameState {
        let stream = self.claim(frame_request, now);
        self.streams[stream].fetch(frame_request.at)
    }

    pub(crate) fn prepare(&mut self, frame_request: &FrameRequest<'_>, now: Instant) {
        self.claim(frame_request, now);
    }

    pub(crate) fn end(&mut self, now: Instant) {
        self.streams
            .retain(|stream| stream.in_use || now.saturating_duration_since(stream.used) < IDLE);
    }

    pub(crate) fn offline(&self) -> BTreeSet<MediaId> {
        self.streams
            .iter()
            .filter(|stream| stream.in_use && stream.is_offline())
            .map(|stream| stream.alignment.media)
            .collect()
    }

    fn claim(&mut self, frame_request: &FrameRequest<'_>, now: Instant) -> usize {
        let reads = |stream: &FootageStream| stream.reads(frame_request.path);
        let reusable = self
            .streams
            .iter()
            .position(|stream| reads(stream) && stream.alignment == frame_request.alignment)
            .or_else(|| {
                self.streams.iter().position(|stream| {
                    reads(stream)
                        && !stream.in_use
                        && stream.alignment.continues(&frame_request.alignment)
                })
            });
        let at = match reusable {
            Some(at) => at,
            None => {
                self.streams.push(FootageStream::open(
                    &self.imaging,
                    frame_request,
                    self.wake.as_ref(),
                    now,
                ));
                self.streams.len() - 1
            }
        };
        self.streams[at].claim(frame_request, now);
        at
    }
}
