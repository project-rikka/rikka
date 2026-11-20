mod crash_report;
mod output;
mod service;
mod shell_log;
mod start_failure;

use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::path::PathBuf;
use std::process::{Child, ExitStatus};
use std::time::{Duration, Instant};

use rikka_compositor_reactor::{Interest, Token};
use rikka_compositor_restarts::{Restart, Restarts};
pub(crate) use rikka_session::desktop::CAPABILITY;
use rikka_session::desktop::Status;
use rustix::process::{Pid, PidfdFlags, Signal, kill_process, pidfd_open};

use crate::context::{SessionCtx, SessionRef, session_ctx, session_ref};
use crate::logging::Journal;
use crate::programs;
use crate::session::Session;
use crate::state::{Compositor, Core};
use crash_report::{Crash, exit_status};
use output::{Flow, Output};
pub(crate) use service::DesktopService;
use shell_log::ShellLog;
use start_failure::StartFailure;

const STOP_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) const LOCKER_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) const FIRST_LOCKER_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub(crate) struct Desktop {
    program: PathBuf,
    process: Option<Process>,
    ever_started: bool,
    locker_deadline: Option<Token>,
    pending_start: Option<Token>,
    restarts: Restarts,
    log: ShellLog,
}

#[derive(Debug)]
struct Process {
    child: Child,
    _pidfd: OwnedFd,
    exit_watch: Token,
    output: Output,
    started: Instant,
    stop: Option<Stop>,
}

#[derive(Debug)]
struct Stop {
    kill_timer: Token,
    restart: bool,
}

#[derive(Debug)]
struct Watches {
    pidfd: OwnedFd,
    exit: Token,
    output: Token,
}

impl Desktop {
    pub(crate) fn new(journal: Journal, socket: &str) -> Desktop {
        Desktop {
            program: programs::shell(),
            process: None,
            ever_started: false,
            locker_deadline: None,
            pending_start: None,
            restarts: Restarts::default(),
            log: ShellLog::new(journal, socket),
        }
    }

    fn is_running(&self) -> bool {
        self.process.is_some()
    }

    fn status(&self) -> Status {
        Status {
            program: self.program.clone(),
            pid: self.process.as_ref().map(|process| process.child.id()),
        }
    }
}

impl Drop for Desktop {
    fn drop(&mut self) {
        if let Some(process) = &self.process {
            signal(&process.child, Signal::TERM);
        }
    }
}

fn signal(child: &Child, signal: Signal) {
    if let Err(error) = kill_process(Pid::from_child(child), signal) {
        tracing::warn!(%error, pid = child.id(), "cannot signal desktop process");
    }
}

fn reap(mut child: Child, signal_to_send: Signal) {
    signal(&child, signal_to_send);
    if let Err(error) = child.wait() {
        tracing::warn!(%error, "cannot reap desktop process");
    }
}

impl SessionCtx<'_> {
    pub(crate) fn start_desktop(&mut self) {
        self.cancel_pending_desktop_start();
        if self.session.desktop.is_running() {
            return tracing::debug!("desktop is already running");
        }
        let program = self.session.desktop.program.clone();
        match session_ref!(self).spawn_desktop() {
            Ok(process) => {
                let pid = process.child.id();
                tracing::info!(program = %program.display(), pid, "desktop started");
                self.session.desktop.log.begin(&program, pid);
                self.session.desktop.process = Some(process);
                self.session.desktop.ever_started = true;
            }
            Err(failure) => {
                tracing::error!(program = %program.display(), "{failure}");
                let guarding = self.shell.session_locked();
                let restart = self
                    .session
                    .desktop
                    .restarts
                    .crashed(Instant::now(), guarding);
                self.schedule_desktop_restart(restart);
            }
        }
    }

    pub(crate) fn stop_desktop(&mut self) {
        self.cancel_pending_desktop_start();
        self.request_desktop_stop(false);
    }

    fn restart_desktop(&mut self) {
        match self.session.desktop.is_running() {
            true => self.request_desktop_stop(true),
            false => self.start_desktop(),
        }
    }

    fn cancel_pending_desktop_start(&mut self) {
        if let Some(pending) = self.session.desktop.pending_start.take() {
            self.core.reactor.remove(pending);
        }
    }

    fn request_desktop_stop(&mut self, restart: bool) {
        let reactor = self.core.reactor.clone();
        let Some(process) = &mut self.session.desktop.process else {
            return;
        };
        if let Some(stop) = &mut process.stop {
            stop.restart = restart;
            return;
        }
        tracing::info!(pid = process.child.id(), "stopping desktop");
        signal(&process.child, Signal::TERM);
        let kill_timer = reactor.after(STOP_TIMEOUT, |compositor: &mut Compositor| {
            compositor.session.kill_desktop()
        });
        process.stop = Some(Stop {
            kill_timer,
            restart,
        });
    }

    fn desktop_output(&mut self) {
        let desktop = &mut self.session.desktop;
        let Some(process) = &mut desktop.process else {
            return;
        };
        if process.output.drain(&mut desktop.log) == Flow::Closed
            && let Some(watch) = process.output.take_watch()
        {
            self.core.reactor.remove(watch);
        }
    }

    fn desktop_exited(&mut self) {
        let Some(mut process) = self.session.desktop.process.take() else {
            return;
        };
        self.core.reactor.remove(process.exit_watch);
        if let Some(stop) = &process.stop {
            self.core.reactor.remove(stop.kill_timer);
        }
        process.output.drain(&mut self.session.desktop.log);
        if let Some(watch) = process.output.take_watch() {
            self.core.reactor.remove(watch);
        }
        let status = process.child.wait();
        match &process.stop {
            Some(stop) => self.desktop_stopped(&status, stop.restart),
            None => self.desktop_crashed(&process, &status),
        }
    }

    fn desktop_stopped(&mut self, status: &io::Result<ExitStatus>, restart: bool) {
        match status {
            Ok(status) => tracing::info!(%status, "desktop exited"),
            Err(error) => tracing::warn!(%error, "cannot reap desktop process"),
        }
        if restart {
            return self.start_desktop();
        }
        if self.session.desktop.locker_deadline.is_some() && session_ref!(self).needs_locker() {
            self.guard_locked_session();
        }
    }

    fn desktop_crashed(&mut self, process: &Process, status: &io::Result<ExitStatus>) {
        let guarding = self.shell.session_locked();
        let desktop = &mut self.session.desktop;
        let restart = desktop.restarts.crashed(Instant::now(), guarding);
        let crash = Crash {
            program: &desktop.program,
            pid: process.child.id(),
            status,
            uptime: process.started.elapsed(),
            restart,
            log: desktop.log.path(),
            output: process.output.tail(),
        };
        let status = exit_status(status);
        match crash.write() {
            Ok(path) => {
                tracing::error!(%status, report = %path.display(), "desktop crashed");
            }
            Err(error) => {
                tracing::error!(%status, "desktop crashed; failed to write crash report: {error}");
            }
        }
        self.schedule_desktop_restart(restart);
    }

    fn schedule_desktop_restart(&mut self, restart: Restart) {
        match restart {
            Restart::After { delay, .. } => {
                tracing::warn!("restarting desktop {restart}");
                let pending = self
                    .core
                    .reactor
                    .after(delay, |compositor: &mut Compositor| {
                        session_ctx!(compositor).start_desktop()
                    });
                self.session.desktop.pending_start = Some(pending);
            }
            Restart::GiveUp => tracing::error!("desktop not restarted: {restart}"),
        }
    }

    pub(crate) fn await_locker(&mut self, timeout: Duration) {
        if let Some(previous) = self.session.desktop.locker_deadline.take() {
            self.core.reactor.remove(previous);
        }
        let deadline = self
            .core
            .reactor
            .after(timeout, |compositor: &mut Compositor| {
                session_ctx!(compositor).locker_deadline_passed()
            });
        self.session.desktop.locker_deadline = Some(deadline);
        if session_ref!(self).needs_locker() && !self.session.desktop.is_running() {
            self.guard_locked_session();
        }
    }

    fn locker_deadline_passed(&mut self) {
        self.session.desktop.locker_deadline = None;
        if !session_ref!(self).needs_locker() {
            return;
        }
        if self.session.desktop.is_running() {
            tracing::warn!("desktop did not lock the session in time; restarting it");
            return self.restart_desktop();
        }
        self.guard_locked_session();
    }

    fn guard_locked_session(&mut self) {
        if self.session.desktop.pending_start.is_some() {
            return;
        }
        tracing::warn!("restarting desktop to guard the locked session");
        self.start_desktop();
    }
}

impl SessionRef<'_> {
    fn spawn_desktop(&self) -> Result<Process, StartFailure> {
        let program = self.session.desktop.program.to_string_lossy();
        let mut command = self.session_command(&program);
        let mut output = Output::attach(&mut command).map_err(StartFailure::Capture)?;
        let child = command.spawn().map_err(StartFailure::Spawn)?;
        drop(command);
        let watches = match self.core.watch_desktop(&child, &output) {
            Ok(watches) => watches,
            Err(error) => {
                reap(child, Signal::KILL);
                return Err(StartFailure::Watch(error));
            }
        };
        output.watched(watches.output);
        Ok(Process {
            child,
            _pidfd: watches.pidfd,
            exit_watch: watches.exit,
            output,
            started: Instant::now(),
            stop: None,
        })
    }

    fn needs_locker(&self) -> bool {
        self.session.desktop.ever_started
            && self.shell.session_locked()
            && self.shell.session_lock().is_none()
    }
}

impl Core {
    fn watch_desktop(&self, child: &Child, output: &Output) -> io::Result<Watches> {
        let pidfd = pidfd_open(Pid::from_child(child), PidfdFlags::empty())?;
        let reactor = &self.reactor;
        let output = reactor.watch(
            output.reader(),
            Interest::READABLE,
            |compositor: &mut Compositor, _| session_ctx!(compositor).desktop_output(),
        )?;
        let exit = reactor.watch(
            pidfd.as_fd(),
            Interest::READABLE,
            |compositor: &mut Compositor, _| session_ctx!(compositor).desktop_exited(),
        );
        match exit {
            Ok(exit) => Ok(Watches {
                pidfd,
                exit,
                output,
            }),
            Err(error) => {
                reactor.remove(output);
                Err(error)
            }
        }
    }
}

impl Session {
    fn kill_desktop(&mut self) {
        let Some(process) = &self.desktop.process else {
            return;
        };
        tracing::warn!(
            pid = process.child.id(),
            timeout_s = STOP_TIMEOUT.as_secs(),
            "desktop did not stop in time; killing it"
        );
        signal(&process.child, Signal::KILL);
    }
}
