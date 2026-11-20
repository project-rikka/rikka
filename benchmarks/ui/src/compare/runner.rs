use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use rikka_session::toplevels::Bounds;
use ui_scene::{FRAME_INTERVAL, PROBE_ENV, ROW_HEIGHT, Recording, Scenario, monotonic_ns};

use super::error::Error;
use super::framework::{Display, Framework, InputSeat};
use super::gpu::GpuProbe;
use super::measurement::{Hover, Idle, Measurement, Startup, Steady};
use super::process::{self, ProcessSample, Usage};
use super::session::{OUTPUT_HEIGHT, OUTPUT_WIDTH, Session, Shown};
use super::stats::FrameTimes;
use super::supervised::Supervised;

const SETTLE: Duration = Duration::from_secs(1);

const STAMP_POLL: Duration = Duration::from_millis(1);

const STAMP_TIMEOUT: Duration = Duration::from_secs(30);

const HOVER_STEP: Duration = FRAME_INTERVAL;

const PARKING_MARGIN: i32 = 2;

const PARKING_CLEARANCE: i32 = 8;

#[derive(Clone, Copy, Debug)]
pub struct Timing {
    pub warm_up: Duration,
    pub window: Duration,
}

pub(crate) struct Runner<'a> {
    session: &'a Session,
    gpu: GpuProbe,
    timing: Timing,
    logs: PathBuf,
    launches: usize,
}

struct Launched {
    child: Supervised,
    recording: Recording,
    program: PathBuf,
    log: PathBuf,
    spawned: u64,
}

struct Window {
    app: Usage,
    servers: Usage,
    wall: Duration,
    stamps: Vec<u64>,
}

impl<'a> Runner<'a> {
    pub(crate) fn new(session: &'a Session, timing: Timing) -> Result<Runner<'a>, Error> {
        let logs = session.root().join("logs");
        std::fs::create_dir_all(&logs).map_err(|error| {
            Error::Compositor(format!("failed to create the log folder: {error}"))
        })?;
        Ok(Runner {
            session,
            gpu: GpuProbe::new(),
            timing,
            logs,
            launches: 0,
        })
    }

    pub(crate) fn measure(
        &mut self,
        framework: Framework,
        scenario: Scenario,
    ) -> Result<Measurement, Error> {
        self.session.wait_until_empty()?;
        let mut launched = self.launch(framework, scenario)?;
        self.drive(&mut launched, framework, scenario)
    }

    fn drive(
        &mut self,
        launched: &mut Launched,
        framework: Framework,
        scenario: Scenario,
    ) -> Result<Measurement, Error> {
        let seat = framework.seat();
        let shown = self.session.wait_for_window(seat)?;
        check_alive(launched)?;
        match scenario {
            Scenario::Startup => self.startup(launched, framework, seat, shown),
            Scenario::Hover => self.hover(launched, seat, shown),
            Scenario::Cells { .. } | Scenario::Scroll => {
                self.park_pointer(seat, shown.bounds)?;
                std::thread::sleep(self.timing.warm_up);
                let window = self.window(launched, |_, deadline| {
                    sleep_until(deadline);
                    Ok(())
                })?;
                check_alive(launched)?;
                let pid = launched.child.id();
                let seconds = window.wall.as_secs_f64();
                let frames = window.stamps.len().max(1) as f64;
                match scenario.animates() {
                    true => Ok(Measurement::Steady(Steady {
                        fps: window.stamps.len() as f64 / seconds,
                        cpu_ms_per_frame: ms(window.app.cpu) / frames,
                        main_thread_ms_per_frame: ms(window.app.main_thread) / frames,
                        cpu_percent: percent(window.app.cpu, window.wall),
                        frames: FrameTimes::from_stamps(&window.stamps, FRAME_INTERVAL),
                        gpu_memory: self.gpu.memory(pid),
                        memory: process::memory(pid)?,
                        server_ms_per_frame: ms(window.servers.cpu) / frames,
                    })),
                    false => Ok(Measurement::Idle(Idle {
                        cpu_percent: percent(window.app.cpu, window.wall),
                        wakeups_per_second: window.app.runs as f64 / seconds,
                        gpu_memory: self.gpu.memory(pid),
                        memory: process::memory(pid)?,
                        server_percent: percent(window.servers.cpu, window.wall),
                    })),
                }
            }
        }
    }

    fn startup(
        &mut self,
        launched: &mut Launched,
        framework: Framework,
        seat: InputSeat,
        shown: Shown,
    ) -> Result<Measurement, Error> {
        let first_frame = match framework.display() {
            Display::Wayland => shown.at,
            Display::X11 => first_frame_seen(launched)?,
        };
        self.park_pointer(seat, shown.bounds)?;
        std::thread::sleep(SETTLE);
        check_alive(launched)?;
        let pid = launched.child.id();
        Ok(Measurement::Startup(Startup {
            first_frame_ms: first_frame.saturating_sub(launched.spawned) as f64 / 1e6,
            window: (shown.bounds.width, shown.bounds.height),
            memory: process::memory(pid)?,
            gpu_memory: self.gpu.memory(pid),
        }))
    }

    fn hover(
        &mut self,
        launched: &mut Launched,
        seat: InputSeat,
        shown: Shown,
    ) -> Result<Measurement, Error> {
        let start = hover_start(shown.bounds);
        self.session.point(seat, start.0, start.1)?;
        std::thread::sleep(self.timing.warm_up);
        let mut moves = 0usize;
        let window = self.window(launched, |session, deadline| {
            moves = sweep(session, seat, shown.bounds, deadline)?;
            Ok(())
        })?;
        check_alive(launched)?;
        let pid = launched.child.id();
        let moves = moves.max(1) as f64;
        Ok(Measurement::Hover(Hover {
            cpu_percent: percent(window.app.cpu, window.wall),
            cpu_ms_per_move: ms(window.app.cpu) / moves,
            main_thread_ms_per_move: ms(window.app.main_thread) / moves,
            memory: process::memory(pid)?,
            server_percent: percent(window.servers.cpu, window.wall),
        }))
    }

    fn window(
        &mut self,
        launched: &Launched,
        during: impl FnOnce(&Session, Instant) -> Result<(), Error>,
    ) -> Result<Window, Error> {
        let pid = launched.child.id();
        let servers = self.session.server_pids();
        let app_before = ProcessSample::take(pid)?;
        let servers_before = servers
            .iter()
            .map(|pid| ProcessSample::take(*pid))
            .collect::<Result<Vec<_>, _>>()?;
        let from = monotonic_ns();
        let started = Instant::now();
        during(self.session, started + self.timing.window)?;
        let until = monotonic_ns();
        let wall = started.elapsed();
        let app = ProcessSample::take(pid)?.since(&app_before);
        let mut servers_used = Usage::default();
        for (pid, before) in servers.iter().zip(&servers_before) {
            servers_used = servers_used + ProcessSample::take(*pid)?.since(before);
        }
        let stamps = launched
            .recording
            .stamps()
            .map_err(Error::Probe)?
            .between(from, until);
        Ok(Window {
            app,
            servers: servers_used,
            wall,
            stamps,
        })
    }

    fn launch(&mut self, framework: Framework, scenario: Scenario) -> Result<Launched, Error> {
        let program = framework.program()?;
        let recording = Recording::new().map_err(Error::Probe)?;
        self.launches += 1;
        let log = self.logs.join(format!(
            "{}-{scenario}-{}.log",
            framework.name(),
            self.launches
        ));
        let spawn_error = |source| Error::Spawn {
            program: program.clone(),
            source,
        };
        let output = std::fs::File::create(&log).map_err(spawn_error)?;
        let errors = output.try_clone().map_err(spawn_error)?;
        let mut command = self.session.command(&program, framework.display());
        command
            .arg(scenario.to_string())
            .envs(framework.environment())
            .env(PROBE_ENV, recording.child_fd().to_string())
            .stdin(Stdio::null())
            .stdout(output)
            .stderr(errors);
        let spawned = monotonic_ns();
        let child = Supervised::spawn(&mut command).map_err(spawn_error)?;
        Ok(Launched {
            child,
            recording,
            program,
            log,
            spawned,
        })
    }

    fn park_pointer(&self, seat: InputSeat, bounds: Bounds) -> Result<(), Error> {
        match bounds.x > PARKING_CLEARANCE && bounds.y > PARKING_CLEARANCE {
            true => self.session.point(seat, PARKING_MARGIN, PARKING_MARGIN),
            false => self.session.point(
                seat,
                OUTPUT_WIDTH as i32 - PARKING_MARGIN,
                OUTPUT_HEIGHT as i32 - PARKING_MARGIN,
            ),
        }
    }
}

impl std::fmt::Debug for Runner<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Runner")
            .field("timing", &self.timing)
            .finish_non_exhaustive()
    }
}

fn first_frame_seen(launched: &mut Launched) -> Result<u64, Error> {
    let started = Instant::now();
    while launched.recording.frames().map_err(Error::Probe)? == 0 {
        check_alive(launched)?;
        if started.elapsed() > STAMP_TIMEOUT {
            return Err(Error::Window(
                "the first frame was never presented".to_owned(),
            ));
        }
        std::thread::sleep(STAMP_POLL);
    }
    Ok(monotonic_ns())
}

fn check_alive(launched: &mut Launched) -> Result<(), Error> {
    match launched.child.has_exited() {
        false => Ok(()),
        true => Err(Error::Exited {
            program: launched.program.clone(),
            log: launched.log.clone(),
        }),
    }
}

fn hover_start(bounds: Bounds) -> (i32, i32) {
    (
        bounds.x + bounds.width / 2,
        bounds.y + (ROW_HEIGHT / 2.0) as i32,
    )
}

fn sweep(
    session: &Session,
    seat: InputSeat,
    bounds: Bounds,
    deadline: Instant,
) -> Result<usize, Error> {
    let (x, top) = hover_start(bounds);
    let rows = ((bounds.height as f32 / ROW_HEIGHT) as i32 - 1).max(1);
    let started = Instant::now();
    let mut moves = 0usize;
    while Instant::now() < deadline {
        let cycle = moves as i32 % (2 * rows);
        let row = match cycle < rows {
            true => cycle,
            false => 2 * rows - cycle,
        };
        session.point(seat, x, top + row * ROW_HEIGHT as i32)?;
        moves += 1;
        sleep_until(started + HOVER_STEP * moves as u32);
    }
    Ok(moves)
}

fn sleep_until(deadline: Instant) {
    if let Some(left) = deadline.checked_duration_since(Instant::now()) {
        std::thread::sleep(left);
    }
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1e3
}

fn percent(cpu: Duration, wall: Duration) -> f64 {
    cpu.as_secs_f64() * 100.0 / wall.as_secs_f64()
}
