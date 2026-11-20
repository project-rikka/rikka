use std::os::fd::{AsFd, BorrowedFd};
use std::sync::Arc;

use rikka_gpu::DrmFormat;

use crate::capture::{Capture, StreamRequest};
use crate::error::{Error, Result};
use crate::frames::{Frame, Latest, NextFrame, Picture};
use crate::portal::{self, CastRequest, Shared};
use crate::source::{SourceKind, Sources};

#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub struct ScreenCast {
    sources: Sources,
    cursor: bool,
    persist: bool,
    restore: Option<String>,
    frame_rate: u32,
}

impl Default for ScreenCast {
    fn default() -> ScreenCast {
        ScreenCast::new()
    }
}

impl ScreenCast {
    pub fn new() -> ScreenCast {
        ScreenCast {
            sources: Sources::ANY,
            cursor: true,
            persist: false,
            restore: None,
            frame_rate: 30,
        }
    }

    pub fn sources(mut self, sources: Sources) -> ScreenCast {
        self.sources = sources;
        self
    }

    pub fn cursor(mut self, shown: bool) -> ScreenCast {
        self.cursor = shown;
        self
    }

    pub fn persist(mut self, persist: bool) -> ScreenCast {
        self.persist = persist;
        self
    }

    pub fn restore_token(mut self, token: impl Into<String>) -> ScreenCast {
        self.restore = Some(token.into());
        self
    }

    pub fn frame_rate(mut self, frame_rate: u32) -> ScreenCast {
        self.frame_rate = frame_rate.clamp(1, 240);
        self
    }

    pub fn start(self) -> Result<Cast> {
        let session = portal::ask(&CastRequest {
            sources: self.sources,
            cursor: self.cursor,
            persist: self.persist,
            restore: self.restore,
        })?;
        let latest = Arc::new(Latest::new().map_err(|error| Error::Signal(error.kind()))?);
        let capture = Capture::start(
            session.remote,
            StreamRequest {
                node: session.node,
                frame_rate: self.frame_rate,
                size: session.size,
            },
            Arc::clone(&latest),
        )?;
        Ok(Cast {
            latest,
            capture,
            _session: session.session,
            kind: session.kind,
            restore: session.restore,
        })
    }
}

pub struct Cast {
    latest: Arc<Latest>,
    capture: Capture,
    _session: Shared,
    kind: SourceKind,
    restore: Option<String>,
}

impl Cast {
    #[must_use]
    pub fn kind(&self) -> SourceKind {
        self.kind
    }

    #[must_use]
    pub fn restore_token(&self) -> Option<&str> {
        self.restore.as_deref()
    }

    #[must_use]
    pub fn next_frame(&self) -> NextFrame {
        self.latest.next()
    }

    pub fn recycle(&self, frame: Frame) {
        if let Picture::Pixels(pixels) = frame.picture {
            self.latest.recycle(pixels.bytes);
        }
    }

    pub fn offer_dmabufs(&self, formats: &[DrmFormat]) {
        self.capture.offer_dmabufs(formats.to_vec());
    }

    #[must_use]
    pub fn has_ended(&self) -> bool {
        self.latest.has_ended()
    }
}

impl AsFd for Cast {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.latest.as_fd()
    }
}

impl std::fmt::Debug for Cast {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Cast")
            .field("kind", &self.kind)
            .field("ended", &self.latest.has_ended())
            .finish_non_exhaustive()
    }
}
