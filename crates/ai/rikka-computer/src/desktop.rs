use rikka_sync::Mutex;
use std::sync::Arc;
use std::time::Duration;

use rikka_capture::{Screen, ScreenWindow, Source};
use rikka_input::Chord;
use rikka_seam::SeamClient;
use rikka_session::agent::{AgentClient, AgentPlacement, Idle, SEAT};
use rikka_types::{Modifiers, PointerButton};

use crate::backend::{Backend, Point};
use crate::display::Display;
use crate::environment::Environment;
use crate::error::{Error, Result, capture};
use crate::input::InputDriver;
use crate::kind::Kind;
use crate::launcher::Launcher;
use crate::region::Region;
use crate::shell::Shell;
use crate::shot::{Screenshot, Shot};
use crate::status::Status;
use crate::wait;
use crate::watcher::Watcher;

#[derive(Debug)]
pub(crate) struct Desktop {
    agent: AgentClient,
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    input: Option<InputDriver>,
    target: Option<Target>,
    cursor_visible: bool,
    launcher: Launcher,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Target {
    output: String,
    size: Point,
}

impl Target {
    fn from_placement(placement: &AgentPlacement) -> Target {
        Target {
            output: placement.output.clone(),
            size: Display::new(placement.width, placement.height).capture_size(),
        }
    }
}

impl State {
    fn input_for(&mut self, target: &Target) -> Result<&mut InputDriver> {
        let input = match self.input.take() {
            Some(mut input) => {
                input.resize(target.size);
                input
            }
            None => InputDriver::for_seat(SEAT, target.size)?,
        };
        Ok(self.input.insert(input))
    }
}

impl Desktop {
    pub(crate) fn new(seam: SeamClient) -> Desktop {
        Desktop {
            agent: AgentClient::new(seam),
            state: Mutex::default(),
        }
    }

    fn with_input<T>(&self, action: impl FnOnce(&mut InputDriver) -> Result<T>) -> Result<T> {
        let target = self.target()?;
        self.show(true)?;
        action(self.state.lock().input_for(&target)?)
    }

    fn target(&self) -> Result<Target> {
        let target = self.state.lock().target.clone();
        match target {
            Some(target) => Ok(target),
            None => self.refresh_target(),
        }
    }

    fn refresh_target(&self) -> Result<Target> {
        let target = Target::from_placement(&self.placement()?);
        self.state.lock().target = Some(target.clone());
        Ok(target)
    }

    fn placement(&self) -> Result<AgentPlacement> {
        self.agent
            .placement()
            .blocking_wait()
            .and_then(|payload| payload.decode::<AgentPlacement>())
            .map_err(|error| Error::Compositor(error.to_string()))
    }
}

impl Backend for Desktop {
    fn kind(&self) -> Kind {
        Kind::Desktop
    }

    fn isolated(self: Arc<Self>) -> Arc<dyn Backend> {
        self
    }

    fn watch(&self, _watcher: Watcher) {}

    fn status(&self) -> Status {
        match &self.state.lock().target {
            Some(target) => Status::open(Display::new(target.size.0, target.size.1), true),
            None => Status::default(),
        }
    }

    fn start(&self, _display: Display, _shell: Shell) -> Result<Status> {
        Err(Error::LiveDesktop)
    }

    fn close(&self) {
        self.state.lock().input = None;
        if let Err(error) = self.show(false) {
            tracing::debug!(%error, "failed to hide agent cursor");
        }
    }

    fn show(&self, visible: bool) -> Result<()> {
        if self.state.lock().cursor_visible == visible {
            return Ok(());
        }
        self.agent
            .show(visible)
            .blocking_wait()
            .map_err(|error| Error::Compositor(error.to_string()))?;
        self.state.lock().cursor_visible = visible;
        Ok(())
    }

    fn screenshot(&self) -> Result<Shot> {
        let target = self.refresh_target()?;
        Screenshot::Output(&Source::Output(target.output)).shot(target.size)
    }

    fn zoom(&self, region: Region) -> Result<Shot> {
        let target = self.target()?;
        Screenshot::Output(&Source::Output(target.output)).zoom(target.size, region)
    }

    fn cursor_position(&self) -> Result<Point> {
        let placement = self.placement()?;
        Ok(cursor_position(
            &placement,
            Target::from_placement(&placement).size,
        ))
    }

    fn move_to(&self, x: u32, y: u32) -> Result<()> {
        self.with_input(|input| input.move_to(x, y))
    }

    fn click(
        &self,
        button: PointerButton,
        count: u32,
        modifiers: Modifiers,
        at: Option<Point>,
    ) -> Result<()> {
        self.with_input(|input| input.click(button, count, modifiers, at))
    }

    fn press(&self, button: PointerButton, at: Option<Point>) -> Result<()> {
        self.with_input(|input| input.press(button, at))
    }

    fn release(&self, button: PointerButton, at: Option<Point>) -> Result<()> {
        self.with_input(|input| input.release(button, at))
    }

    fn drag(&self, button: PointerButton, to: Point, modifiers: Modifiers) -> Result<()> {
        self.with_input(|input| input.drag(button, to, modifiers))
    }

    fn scroll(&self, dx: f64, dy: f64, modifiers: Modifiers, at: Option<Point>) -> Result<()> {
        self.with_input(|input| input.scroll(dx, dy, modifiers, at))
    }

    fn hold_modifiers(&self, modifiers: Modifiers) -> Result<()> {
        self.with_input(|input| input.hold_modifiers(modifiers))
    }

    fn key(&self, chord: Chord) -> Result<()> {
        self.with_input(|input| input.key(chord))
    }

    fn hold_key(&self, chord: Chord, duration: Duration) -> Result<()> {
        self.with_input(|input| input.hold_key(chord, duration))
    }

    fn type_text(&self, text: &str) -> Result<()> {
        self.with_input(|input| input.type_text(text))
    }

    fn launch(&self, command: &str) -> Result<u32> {
        self.state.lock().launcher.launch(command, None, 0)
    }

    fn windows(&self) -> Result<Vec<ScreenWindow>> {
        Ok(Screen::connect().map_err(capture)?.windows())
    }

    fn wait_for_window(
        &self,
        matches: &dyn Fn(&ScreenWindow) -> bool,
        timeout: Duration,
    ) -> Result<ScreenWindow> {
        wait::wait_for_window(Screen::connect().map_err(capture)?, matches, timeout)
    }

    fn wait_idle(&self, timeout: Duration) -> Result<()> {
        let idle = self
            .agent
            .wait_idle(wait::QUIET_PERIOD, timeout)
            .blocking_wait()
            .and_then(|payload| payload.decode::<Idle>())
            .map_err(|error| Error::Compositor(error.to_string()))?;
        match idle {
            Idle::Quiescent => Ok(()),
            Idle::TimedOut => Err(Error::IdleTimeout(timeout)),
        }
    }

    fn environment(&self) -> Option<Environment> {
        None
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "scales a logical output position into the clamped screenshot size"
)]
fn cursor_position(placement: &AgentPlacement, size: Point) -> Point {
    let scale = |position: f64, extent: u32, target: u32| -> u32 {
        if extent == 0 {
            return 0;
        }
        ((position / f64::from(extent)).clamp(0.0, 1.0) * f64::from(target)) as u32
    };
    (
        scale(placement.x, placement.width, size.0).min(size.0.saturating_sub(1)),
        scale(placement.y, placement.height, size.1).min(size.1.saturating_sub(1)),
    )
}
