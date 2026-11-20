use std::io;
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

use drm::control::AtomicCommitFlags;
use rikka_compositor_reactor::Timestamp;
use rikka_compositor_types::Flip;

use super::decide::{AsyncSupport, Content, Decision, WakeReason, arrived, decide, eligible};
use super::screen::{Frame, InFlightCommit, Queued};
use super::worker::{Worker, paused_by};
use crate::blob::Blob;
use crate::cursor::CursorImage;
use crate::damage::Damage;
use crate::drm_fd::DrmFd;
use crate::event::{KmsEvent, Rejection};
use crate::frame_commit::{Config, FrameCommit, PlaneUpdate};
use crate::ids::CrtcId;
use crate::present::{Fence, Present};
use crate::rect::Rect;

const RETRY: Duration = Duration::from_millis(1);

struct Attempt {
    commit: FrameCommit,
    frame: Option<Frame>,
    fence: Option<Fence>,
    cursor: Option<Option<CursorImage>>,
    asynchronous: bool,
    restore: bool,
    _damage: Option<Blob>,
}

impl Attempt {
    fn tears(&self) -> bool {
        self.asynchronous && self.frame.is_some()
    }

    fn flags(&self) -> AtomicCommitFlags {
        let mut flags = AtomicCommitFlags::NONBLOCK | AtomicCommitFlags::PAGE_FLIP_EVENT;
        if self.commit.is_modeset() {
            flags |= AtomicCommitFlags::ALLOW_MODESET;
        }
        if self.asynchronous {
            flags |= AtomicCommitFlags::PAGE_FLIP_ASYNC;
        }
        flags
    }
}

enum TestResult {
    Passed,
    WithoutCursor,
    Failed(io::Error),
}

impl Worker {
    pub(super) fn schedule(&mut self, crtc: CrtcId) {
        self.schedule_as(crtc, WakeReason::Requested);
    }

    fn schedule_as(&mut self, crtc: CrtcId, wake: WakeReason) {
        if self.paused {
            return;
        }
        let support = self.async_support();
        let moved = self.cursors.pending(crtc);
        let Some(screen) = self.screens.get(&crtc) else {
            return;
        };
        let decision = decide(&screen.commit_state(Timestamp::now(), support, moved));
        let decision = match wake {
            WakeReason::Requested => decision,
            WakeReason::Deadline => arrived(decision),
        };
        match decision {
            Decision::Wait => self.cancel_deadline(crtc),
            Decision::Now => {
                self.cancel_deadline(crtc);
                self.commit(crtc);
            }
            Decision::At(at) => self.wake_at(crtc, at),
        }
    }

    fn cancel_deadline(&mut self, crtc: CrtcId) {
        if let Some((token, _)) = self
            .screens
            .get_mut(&crtc)
            .and_then(|screen| screen.deadline.take())
        {
            self.reactor.remove(token);
        }
    }

    fn wake_at(&mut self, crtc: CrtcId, at: Timestamp) {
        let Some(screen) = self.screens.get_mut(&crtc) else {
            return;
        };
        if let Some((token, planned)) = screen.deadline {
            if planned == at {
                return;
            }
            self.reactor.remove(token);
        }
        let token = self
            .reactor
            .timer(at.to_instant(), move |worker: &mut Worker| {
                worker.deadline_reached(crtc, at);
            });
        screen.deadline = Some((token, at));
    }

    fn deadline_reached(&mut self, crtc: CrtcId, planned: Timestamp) {
        let Some(screen) = self.screens.get_mut(&crtc) else {
            return;
        };
        screen.deadline = None;
        screen.lead.woke(Timestamp::now().saturating_since(planned));
        self.schedule_as(crtc, WakeReason::Deadline);
    }

    pub(super) fn async_support(&self) -> AsyncSupport {
        let flips = self.hardware.caps.async_flips;
        AsyncSupport {
            flips,
            cursor: flips && self.async_cursor != Some(false),
        }
    }

    fn commit(&mut self, crtc: CrtcId) {
        let started = Instant::now();
        let Some(mut attempt) = self.prepare(crtc) else {
            return;
        };
        let modeset = attempt.commit.is_modeset();
        if attempt.tears()
            && self
                .screens
                .get_mut(&crtc)
                .is_some_and(|screen| screen.async_refused.lookup(&attempt.commit.config(true)))
        {
            attempt.asynchronous = false;
        }
        let config = match self.validate(crtc, &mut attempt) {
            Ok(config) => config,
            Err(error) => {
                self.refused(crtc, attempt, error);
                return;
            }
        };

        let result = self.submit_commit(&attempt.commit, attempt.flags());
        let took = started.elapsed();
        match result {
            Ok(()) => {
                drop(attempt.fence);
                let Some(screen) = self.screens.get_mut(&crtc) else {
                    return;
                };
                let refresh = screen.setup.mode.refresh();
                screen.lead.committed(took, refresh);
                if !modeset {
                    screen.validated.insert(config);
                }
                if attempt.asynchronous {
                    screen.last_async = Some(Timestamp::now());
                }
                screen.modeset = false;
                screen.vrr_dirty = false;
                if attempt.cursor.is_some() {
                    screen.cursor.dirty = false;
                }
                screen.in_flight = Some(InFlightCommit {
                    frame: attempt.frame,
                    cursor: attempt.cursor,
                    asynchronous: attempt.asynchronous,
                });
            }
            Err(error) => self.failed(crtc, attempt, error),
        }
    }

    fn validate(&mut self, crtc: CrtcId, attempt: &mut Attempt) -> io::Result<Config> {
        let config = attempt.commit.config(attempt.asynchronous);
        let validated = !attempt.commit.is_modeset()
            && self
                .screens
                .get_mut(&crtc)
                .is_some_and(|screen| screen.validated.lookup(&config));
        if validated {
            return Ok(config);
        }
        let flags = attempt.flags();
        match self.test(&mut attempt.commit, flags) {
            TestResult::Passed => Ok(config),
            TestResult::WithoutCursor => {
                self.cursor_refused(crtc);
                attempt.cursor = None;
                Ok(attempt.commit.config(attempt.asynchronous))
            }
            TestResult::Failed(error) if attempt.tears() => {
                tracing::debug!("{crtc} refused an async page flip, flipping on vblank: {error}");
                if let Some(screen) = self.screens.get_mut(&crtc) {
                    screen.async_refused.insert(config);
                }
                attempt.asynchronous = false;
                self.validate(crtc, attempt)
            }
            TestResult::Failed(error) if attempt.commit.changes_vrr() => {
                tracing::info!("{crtc} cannot switch variable refresh without a modeset: {error}");
                self.vrr_refused(crtc);
                attempt.commit.vrr = None;
                self.validate(crtc, attempt)
            }
            TestResult::Failed(error) => Err(error),
        }
    }

    fn vrr_refused(&mut self, crtc: CrtcId) {
        let Some(screen) = self.screens.get_mut(&crtc) else {
            return;
        };
        screen.setup.vrr = !screen.setup.vrr;
        screen.vrr_dirty = false;
        screen.vrr_fixed = true;
        screen.validated.clear();
        screen.publish();
    }

    fn prepare(&mut self, crtc: CrtcId) -> Option<Attempt> {
        let now = Timestamp::now();
        let support = self.async_support();
        let screen = self.screens.get_mut(&crtc)?;
        screen.follow_cursor(&self.cursors);
        let commit_state = screen.commit_state(now, support, false);
        let ready = screen.queue.iter().rposition(|queued| {
            queued.signaled
                && eligible(
                    queued.present.not_before,
                    queued.present.flip,
                    &commit_state,
                )
        });
        let late = commit_state.target().is_some()
            && !commit_state.cursor_dirty
            && screen.setup.primary.in_fence.is_some();
        let assignment = ready.or_else(|| {
            screen
                .queue
                .iter()
                .rposition(|queued| {
                    eligible(
                        queued.present.not_before,
                        queued.present.flip,
                        &commit_state,
                    ) && queued
                        .present
                        .fence
                        .as_ref()
                        .is_none_or(|fence| fence.sync_file().is_some())
                })
                .filter(|_| late)
        });
        if screen.modeset && assignment.is_none() && screen.front.is_none() {
            return None;
        }
        let taken = match assignment {
            Some(superseded) => {
                for _ in 0..superseded {
                    let queued = self.screens.get_mut(&crtc)?.queue.pop_front()?;
                    self.drop_queued(crtc, queued.present, queued.watch);
                }
                self.screens.get_mut(&crtc)?.queue.pop_front()
            }
            None => None,
        };
        if let Some(token) = taken.as_ref().and_then(|queued| queued.watch) {
            self.reactor.remove(token);
        }

        let screen = self.screens.get(&crtc)?;
        let (frame, fence, damage) = match taken {
            Some(queued) => {
                let (frame, fence, damage) = Frame::take(queued.present);
                (Some(frame), fence, damage)
            }
            None => (None, None, Damage::default()),
        };
        let restore = frame.is_none() && screen.modeset;
        let shown = match (&frame, restore) {
            (Some(frame), _) => Some(&frame.primary),
            (None, true) => screen.front.as_ref().map(|front| &front.primary),
            (None, false) => None,
        };
        let tears = commit_state.async_content
            && frame
                .as_ref()
                .is_some_and(|frame| frame.flip == Flip::Async);
        let with_cursor = !tears && (screen.cursor.dirty || screen.modeset);
        let (mut commit, cursor) = screen.commit(shown, with_cursor);
        let damage_blob = match (&frame, screen.setup.primary.damage_clips) {
            (Some(frame), Some(_)) => damaged(&self.fd, &damage, frame),
            _ => None,
        };
        if frame.is_some()
            && let Some(PlaneUpdate::Show(primary)) = &mut commit.primary
        {
            primary.damage = damage_blob.as_ref().map(Blob::id);
            primary.fence = fence
                .as_ref()
                .and_then(Fence::sync_file)
                .map(AsRawFd::as_raw_fd);
        }
        if commit.is_empty() {
            return None;
        }
        let asynchronous = !commit.is_modeset()
            && (tears
                || (frame.is_none()
                    && !restore
                    && commit_state.async_cursor
                    && commit_state.content == Content::None));
        Some(Attempt {
            commit,
            frame,
            fence,
            cursor,
            asynchronous,
            restore,
            _damage: damage_blob,
        })
    }

    pub(super) fn pretest(&mut self, crtc: CrtcId) {
        let moved = self.cursors.pending(crtc);
        let support = self.async_support();
        let Some(screen) = self.screens.get_mut(&crtc) else {
            return;
        };
        let Some(queued) = screen.queue.back().filter(|_| !screen.modeset) else {
            return;
        };
        let tears = queued.present.flip == Flip::Async
            && screen
                .commit_state(Timestamp::now(), support, moved)
                .async_content;
        let with_cursor = !tears && (screen.cursor.dirty || moved);
        let (commit, _) = screen.commit(Some(&queued.present.primary), with_cursor);
        let config = commit.config(tears);
        if screen.validated.lookup(&config) || screen.async_refused.lookup(&config) {
            return;
        }
        self.request.clear();
        commit.encode(&mut self.request);
        let mut flags = AtomicCommitFlags::NONBLOCK | AtomicCommitFlags::PAGE_FLIP_EVENT;
        if tears {
            flags |= AtomicCommitFlags::PAGE_FLIP_ASYNC;
        }
        match (self.request.submit(&self.fd, testing(flags)), tears) {
            (Ok(()), _) => screen.validated.insert(config),
            (Err(_), true) => screen.async_refused.insert(config),
            (Err(_), false) => {}
        }
    }

    fn test(&mut self, commit: &mut FrameCommit, flags: AtomicCommitFlags) -> TestResult {
        let flags = testing(flags);
        let Err(error) = self.submit_commit(commit, flags) else {
            return TestResult::Passed;
        };
        match commit.without_cursor() {
            Some(rest) if self.submit_commit(&rest, flags).is_ok() => {
                *commit = rest;
                TestResult::WithoutCursor
            }
            _ => TestResult::Failed(error),
        }
    }

    fn cursor_refused(&mut self, crtc: CrtcId) {
        if let Some(screen) = self.screens.get_mut(&crtc) {
            tracing::info!("{crtc} rejected the cursor plane");
            screen.cursor.rejected = true;
            screen.cursor.dirty = false;
        }
        let _ = self.events.send(KmsEvent::CursorRejected { crtc });
    }

    fn refused(&mut self, crtc: CrtcId, attempt: Attempt, error: io::Error) {
        if attempt.asynchronous && attempt.frame.is_none() {
            tracing::debug!("async cursor updates unsupported: {error}");
            self.async_cursor = Some(false);
            self.schedule(crtc);
            return;
        }
        let modeset = attempt.commit.is_modeset();
        match attempt.frame {
            Some(frame) => {
                let reason = match (modeset, frame.zero_copy) {
                    (true, _) => Rejection::Modeset,
                    (false, true) => Rejection::Scanout,
                    (false, false) => Rejection::Commit,
                };
                match reason {
                    Rejection::Scanout => {
                        tracing::debug!(?reason, "{crtc} rejected a frame: {error}");
                    }
                    Rejection::Modeset | Rejection::Commit => {
                        tracing::warn!(
                            ?reason,
                            errno = error.raw_os_error(),
                            "{crtc} rejected a frame: {error}"
                        );
                    }
                }
                let _ = self.events.send(KmsEvent::Rejected {
                    crtc,
                    frame: frame.id,
                    reason,
                });
                self.retire(crtc, frame, false);
                self.schedule(crtc);
            }
            None if attempt.restore => {
                tracing::warn!("failed to re-enable {crtc}: {error}");
            }
            None => {
                self.cursor_refused(crtc);
                self.schedule(crtc);
            }
        }
    }

    fn failed(&mut self, crtc: CrtcId, attempt: Attempt, error: io::Error) {
        if paused_by(&error) {
            tracing::info!("{crtc} lost DRM master");
            self.paused = true;
            if let Some(frame) = attempt.frame {
                self.retire(crtc, frame, false);
            }
            return;
        }
        match error.raw_os_error() {
            Some(libc::EBUSY) => {
                if let Some(frame) = attempt.frame {
                    self.requeue(crtc, frame, attempt.fence);
                }
                self.wake_at(crtc, Timestamp::now() + RETRY);
            }
            Some(libc::ENOENT) => {
                tracing::info!("{crtc} vanished with a frame in flight");
                if let Some(frame) = attempt.frame {
                    self.retire(crtc, frame, false);
                }
                if let Some(screen) = self.screens.remove(&crtc) {
                    self.release(screen);
                }
                let _ = self.events.send(KmsEvent::Lost { crtc });
            }
            _ if attempt.tears() => {
                tracing::debug!("{crtc} refused an async page flip that passed TEST_ONLY: {error}");
                if let Some(screen) = self.screens.get_mut(&crtc) {
                    screen.validated.clear();
                    screen.async_refused.insert(attempt.commit.config(true));
                }
                if let Some(frame) = attempt.frame {
                    self.requeue(crtc, frame, attempt.fence);
                }
                self.schedule(crtc);
            }
            _ => {
                tracing::warn!("{crtc} rejected a commit that passed TEST_ONLY: {error}");
                if let Some(screen) = self.screens.get_mut(&crtc) {
                    screen.validated.clear();
                }
                if let Some(frame) = attempt.frame {
                    let _ = self.events.send(KmsEvent::Rejected {
                        crtc,
                        frame: frame.id,
                        reason: Rejection::Commit,
                    });
                    self.retire(crtc, frame, false);
                }
            }
        }
    }

    fn requeue(&mut self, crtc: CrtcId, frame: Frame, fence: Option<Fence>) {
        let Some(screen) = self.screens.get_mut(&crtc) else {
            self.retire(crtc, frame, false);
            return;
        };
        let mut present = Present::new(crtc, frame.id, frame.primary).flip(frame.flip);
        present.fence = fence;
        screen.queue.push_front(Queued {
            present,
            watch: None,
            signaled: true,
        });
    }
}

fn damaged(fd: &DrmFd, damage: &Damage, frame: &Frame) -> Option<Blob> {
    let whole = Rect::sized(frame.primary.width(), frame.primary.height());
    let rects = damage.rects();
    if rects.is_empty() || rects.contains(&whole) {
        return None;
    }
    Blob::damage(fd, damage)
        .inspect_err(|error| tracing::debug!("failed to create damage blob: {error}"))
        .ok()
}

fn testing(flags: AtomicCommitFlags) -> AtomicCommitFlags {
    flags.difference(AtomicCommitFlags::PAGE_FLIP_EVENT) | AtomicCommitFlags::TEST_ONLY
}
