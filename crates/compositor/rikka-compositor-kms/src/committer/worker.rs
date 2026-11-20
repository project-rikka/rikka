use std::collections::BTreeMap;
use std::io;
use std::ops::Bound;
use std::os::fd::AsFd;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use drm::control::{AtomicCommitFlags, Device as _, Event};
use rikka_compositor_reactor::{Interest, Reactor, Sender, Timestamp, Token};

use super::command::{Command, Hardware, Setup};
use super::realtime;
use super::screen::{Cursor, Frame, InFlightCommit, Screen};
use super::slots::CursorSlots;
use super::stamp::vblank_at;
use crate::atomic::AtomicRequest;
use crate::cursor::CursorImage;
use crate::drm_fd::DrmFd;
use crate::error::{Error, Result};
use crate::event::{KmsEvent, Presented, PresentedFlags};
use crate::frame_commit::FrameCommit;
use crate::ids::CrtcId;
use crate::plan::Plan;
use crate::present::Present;
use crate::properties::ConnectorProps;

const QUEUE_LIMIT: usize = 4;

pub(super) struct Worker {
    pub(super) fd: DrmFd,
    pub(super) reactor: Reactor<Worker>,
    pub(super) hardware: Hardware,
    pub(super) cursors: Arc<CursorSlots>,
    pub(super) events: Sender<KmsEvent>,
    pub(super) screens: BTreeMap<CrtcId, Screen>,
    pub(super) request: AtomicRequest,
    pub(super) paused: bool,
    pub(super) stopped: bool,
    pub(super) async_cursor: Option<bool>,
}

pub(super) fn run(
    fd: DrmFd,
    hardware: Hardware,
    cursors: Arc<CursorSlots>,
    events: Sender<KmsEvent>,
    ready: mpsc::SyncSender<Result<Sender<Command>>>,
) {
    realtime::make_realtime();
    let started = start(fd, hardware, cursors, events);
    let (reactor, mut worker, commands) = match started {
        Ok(started) => started,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    worker.reset();
    if ready.send(Ok(commands)).is_err() {
        return;
    }
    if let Err(error) = reactor.run(&mut worker, |worker| worker.stopped) {
        tracing::error!("KMS thread stopped: {error}");
    }
}

fn start(
    fd: DrmFd,
    hardware: Hardware,
    cursors: Arc<CursorSlots>,
    events: Sender<KmsEvent>,
) -> Result<(Reactor<Worker>, Worker, Sender<Command>)> {
    let reactor = Reactor::new().map_err(Error::Thread)?;
    let commands = reactor
        .mailbox(|worker: &mut Worker, command| worker.command(command))
        .map_err(Error::Thread)?;
    reactor
        .watch(fd.as_fd(), Interest::READABLE, |worker: &mut Worker, _| {
            worker.drain_events();
        })
        .map_err(Error::Thread)?;
    reactor
        .watch(
            cursors.as_fd(),
            Interest::READABLE,
            |worker: &mut Worker, _| worker.cursor_moved(),
        )
        .map_err(Error::Thread)?;
    let worker = Worker {
        fd,
        reactor: reactor.clone(),
        hardware,
        cursors,
        events,
        screens: BTreeMap::new(),
        request: AtomicRequest::default(),
        paused: false,
        stopped: false,
        async_cursor: None,
    };
    Ok((reactor, worker, commands))
}

impl Worker {
    fn command(&mut self, command: Command) {
        match command {
            Command::Enable(setup) => self.enable(*setup),
            Command::Disable(crtc) => self.disable(crtc),
            Command::Vrr { crtc, enabled } => self.set_vrr(crtc, enabled),
            Command::Present(present) => self.present(present),
            Command::Cursor { crtc, image } => self.cursor_image(crtc, image),
            Command::Pause => self.pause(),
            Command::Resume(connectors) => self.resume(connectors),
            Command::Stop => self.stopped = true,
        }
    }

    fn enable(&mut self, setup: Setup) {
        let crtc = setup.crtc;
        let (cursor, flipping) = match self.screens.remove(&crtc) {
            Some(old) => {
                let flipping = old.in_flight.is_some();
                (self.release(old), flipping)
            }
            None => (
                Cursor {
                    at: self.cursors.position(crtc),
                    ..Cursor::default()
                },
                false,
            ),
        };
        let mut screen = Screen::new(setup, cursor);
        if flipping {
            screen.in_flight = Some(InFlightCommit {
                frame: None,
                cursor: None,
                asynchronous: true,
            });
        }
        screen.publish();
        self.screens.insert(crtc, screen);
    }

    fn disable(&mut self, crtc: CrtcId) {
        let Some(screen) = self.screens.remove(&crtc) else {
            return;
        };
        let mut plan = Plan::new();
        plan.switch_off(&screen.setup.crtc_props);
        plan.detach(&screen.setup.connector);
        for plane in [
            Some(screen.setup.primary),
            screen.setup.cursor,
            screen.setup.overlay,
        ]
        .into_iter()
        .flatten()
        {
            plan.hide(&plane);
        }
        if !self.paused
            && let Err(error) = self.submit_plan(&plan)
        {
            tracing::warn!("failed to disable {crtc}: {error}");
        }
        self.release(screen);
    }

    pub(super) fn release(&mut self, mut screen: Screen) -> Cursor {
        if let Some((token, _)) = screen.deadline.take() {
            self.reactor.remove(token);
        }
        let crtc = screen.crtc();
        while let Some(queued) = screen.queue.pop_front() {
            self.drop_queued(crtc, queued.present, queued.watch);
        }
        if let Some(frame) = screen
            .in_flight
            .take()
            .and_then(|in_flight| in_flight.frame)
        {
            self.retire(crtc, frame, false);
        }
        if let Some(frame) = screen.front.take() {
            self.retire(crtc, frame, true);
        }
        Cursor {
            image: screen.cursor.image,
            at: screen.cursor.at,
            ..Cursor::default()
        }
    }

    fn set_vrr(&mut self, crtc: CrtcId, enabled: bool) {
        let Some(screen) = self.screens.get_mut(&crtc) else {
            return;
        };
        if screen.setup.vrr == enabled {
            return;
        }
        if screen.vrr_fixed {
            tracing::debug!("{crtc} keeps variable refresh at {}", screen.setup.vrr);
            return;
        }
        let has_property = screen.setup.crtc_props.vrr_enabled.is_some();
        screen.setup.vrr = enabled && has_property;
        screen.vrr_dirty = has_property;
        screen.validated.clear();
        screen.publish();
    }

    fn present(&mut self, present: Present) {
        let crtc = present.crtc;
        if self.paused || !self.screens.contains_key(&crtc) {
            self.drop_queued(crtc, present, None);
            return;
        }
        let frame = present.frame;
        let signaled = present
            .fence
            .as_ref()
            .is_none_or(|fence| signaled(fence.fd()));
        let watch = match (&present.fence, signaled) {
            (Some(fence), false) => self
                .reactor
                .watch(
                    fence.fd().as_fd(),
                    Interest::READABLE,
                    move |worker: &mut Worker, _| {
                        worker.fence_signaled(crtc, frame);
                    },
                )
                .inspect_err(|error| tracing::warn!("failed to watch fence: {error}"))
                .ok(),
            _ => None,
        };
        let unguarded = !signaled
            && watch.is_none()
            && present
                .fence
                .as_ref()
                .is_some_and(|fence| fence.sync_file().is_none());
        if unguarded {
            self.drop_queued(crtc, present, None);
            return;
        }
        let overflow = {
            let screen = self.screens.get_mut(&crtc).expect("checked above");
            screen.queue.push_back(super::screen::Queued {
                present,
                watch,
                signaled: signaled || watch.is_none(),
            });
            (screen.queue.len() > QUEUE_LIMIT)
                .then(|| screen.queue.pop_front())
                .flatten()
        };
        if let Some(oldest) = overflow {
            self.drop_queued(crtc, oldest.present, oldest.watch);
        }
        self.pretest(crtc);
        self.schedule(crtc);
    }

    fn fence_signaled(&mut self, crtc: CrtcId, frame: u64) {
        let Some(screen) = self.screens.get_mut(&crtc) else {
            return;
        };
        let Some(queued) = screen
            .queue
            .iter_mut()
            .find(|queued| queued.present.frame == frame)
        else {
            return;
        };
        queued.signaled = true;
        if let Some(token) = queued.watch.take() {
            self.reactor.remove(token);
        }
        self.schedule(crtc);
    }

    fn cursor_moved(&mut self) {
        self.cursors.drain();
        let mut next = self.screens.keys().next().copied();
        while let Some(crtc) = next {
            if self.cursors.pending(crtc) {
                self.schedule(crtc);
            }
            next = self
                .screens
                .range((Bound::Excluded(crtc), Bound::Unbounded))
                .next()
                .map(|(crtc, _)| *crtc);
        }
    }

    fn cursor_image(&mut self, crtc: CrtcId, image: Option<CursorImage>) {
        let Some(screen) = self.screens.get_mut(&crtc) else {
            return;
        };
        screen.cursor.image = image;
        screen.cursor.dirty = true;
        self.schedule(crtc);
    }

    fn pause(&mut self) {
        self.paused = true;
        let crtcs: Vec<CrtcId> = self.screens.keys().copied().collect();
        for crtc in crtcs {
            let screen = self.screens.get_mut(&crtc).expect("listed above");
            let deadline = screen.deadline.take();
            let queue = std::mem::take(&mut screen.queue);
            if let Some((token, _)) = deadline {
                self.reactor.remove(token);
            }
            for queued in queue {
                self.drop_queued(crtc, queued.present, queued.watch);
            }
        }
    }

    fn resume(&mut self, connectors: Vec<ConnectorProps>) {
        self.paused = false;
        self.hardware.connectors = connectors;
        let mut plan = self.reset_plan();
        let crtcs: Vec<CrtcId> = self.screens.keys().copied().collect();
        let mut restored = Vec::new();
        for crtc in &crtcs {
            let screen = self.screens.get_mut(crtc).expect("listed above");
            if let Some(frame) = screen
                .in_flight
                .take()
                .and_then(|in_flight| in_flight.frame)
                && let Some(old) = screen.front.replace(frame)
            {
                self.retire(*crtc, old, true);
            }
            let screen = self.screens.get_mut(crtc).expect("listed above");
            screen.modeset = true;
            screen.validated.clear();
            screen.cursor.dirty = true;
            screen.last_vblank = None;
            screen.follow_cursor(&self.cursors);
            let Some(front) = &screen.front else {
                continue;
            };
            let (commit, visible) = screen.commit(Some(&front.primary), true);
            plan.apply(&commit);
            restored.push((*crtc, visible.flatten()));
        }
        match self.submit_plan(&plan) {
            Ok(()) => {
                for (crtc, visible) in restored {
                    let screen = self.screens.get_mut(&crtc).expect("listed above");
                    screen.modeset = false;
                    screen.cursor.dirty = false;
                    screen.cursor.shown = visible;
                }
            }
            Err(error) => tracing::warn!("failed to restore outputs: {error}"),
        }
        for crtc in crtcs {
            if let Some(screen) = self.screens.get(&crtc) {
                screen.publish();
            }
            self.schedule(crtc);
        }
    }

    pub(super) fn reset(&mut self) {
        let plan = self.reset_plan();
        match self.submit_plan(&plan) {
            Ok(()) => tracing::debug!("disabled all outputs before the first modeset"),
            Err(error) => {
                tracing::warn!("failed to reset outputs: {error}");
                self.paused = paused_by(&error);
            }
        }
    }

    fn reset_plan(&self) -> Plan {
        let mut plan = Plan::new();
        for crtc in &self.hardware.crtcs {
            plan.switch_off(crtc);
        }
        for plane in &self.hardware.planes {
            plan.hide(plane);
        }
        for connector in &self.hardware.connectors {
            plan.detach(connector);
        }
        plan
    }

    fn drain_events(&mut self) {
        let events = match self.fd.receive_events() {
            Ok(events) => events,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return,
            Err(error) => {
                tracing::warn!("failed to read DRM events: {error}");
                return;
            }
        };
        for event in events {
            if let Event::PageFlip(flip) = event {
                self.flipped(CrtcId::from_handle(flip.crtc), flip.frame, flip.duration);
            }
        }
    }

    fn flipped(&mut self, crtc: CrtcId, counter: u32, time: Duration) {
        let monotonic = self.hardware.caps.monotonic_timestamps;
        let Some(screen) = self.screens.get_mut(&crtc) else {
            return;
        };
        let Some(in_flight) = screen.in_flight.take() else {
            return;
        };
        let now = Timestamp::now();
        let reported = monotonic.then(|| Timestamp::from_monotonic(time));
        let refresh = screen.setup.mode.refresh();
        let at = match in_flight.asynchronous {
            true => now,
            false => vblank_at(reported, now, refresh),
        };
        let sequence = match in_flight.asynchronous {
            true => screen.last_sequence,
            false => {
                let sequence = screen.sequence.extend(counter, at, refresh);
                screen.last_vblank = Some(at);
                screen.last_sequence = sequence;
                sequence
            }
        };
        screen.publish();
        if let Some(cursor) = in_flight.cursor {
            screen.cursor.shown = cursor;
        }
        if let Some(frame) = in_flight.frame {
            let mut flags = PresentedFlags::HW_COMPLETION;
            if !in_flight.asynchronous {
                flags |= PresentedFlags::VSYNC;
            }
            if monotonic && !in_flight.asynchronous {
                flags |= PresentedFlags::HW_CLOCK;
            }
            if frame.zero_copy {
                flags |= PresentedFlags::ZERO_COPY;
            }
            let presented = Presented {
                crtc,
                frame: frame.id,
                sequence,
                at,
                refresh: screen.refresh(),
                flags,
            };
            let retired = screen
                .front
                .replace(frame)
                .map(|old| old.retired(crtc, true));
            let _ = self.events.send(KmsEvent::Flipped { presented, retired });
        }
        self.schedule(crtc);
    }

    pub(super) fn drop_queued(&mut self, crtc: CrtcId, present: Present, watch: Option<Token>) {
        if let Some(token) = watch {
            self.reactor.remove(token);
        }
        let (frame, _, _) = Frame::take(present);
        self.retire(crtc, frame, false);
    }

    pub(super) fn retire(&self, crtc: CrtcId, frame: Frame, shown: bool) {
        let _ = self
            .events
            .send(KmsEvent::Retired(frame.retired(crtc, shown)));
    }

    pub(super) fn submit_plan(&mut self, plan: &Plan) -> io::Result<()> {
        self.request.clear();
        plan.encode(&mut self.request);
        self.request.submit(&self.fd, plan.flags())
    }

    pub(super) fn submit_commit(
        &mut self,
        commit: &FrameCommit,
        flags: AtomicCommitFlags,
    ) -> io::Result<()> {
        self.request.clear();
        commit.encode(&mut self.request);
        self.request.submit(&self.fd, flags)
    }
}

pub(super) fn paused_by(error: &io::Error) -> bool {
    matches!(error.raw_os_error(), Some(libc::EACCES) | Some(libc::EPERM))
}

fn signaled(fence: &std::os::fd::OwnedFd) -> bool {
    let mut fds = [rustix::event::PollFd::new(
        fence,
        rustix::event::PollFlags::IN,
    )];
    let zero = rustix::time::Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    matches!(rustix::event::poll(&mut fds, Some(&zero)), Ok(count) if count > 0)
}
