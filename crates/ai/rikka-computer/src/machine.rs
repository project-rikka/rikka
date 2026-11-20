use rikka_sync::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use rikka_capture::{Screen, ScreenWindow, Source};
use rikka_input::Chord;
use rikka_types::{Modifiers, PointerButton};

use crate::backend::{Backend, Point};
use crate::display::Display;
use crate::environment::Environment;
use crate::error::{Error, Result, capture};
use crate::frame_stream::FrameStream;
use crate::input::InputDriver;
use crate::kind::Kind;
use crate::presence::Presence;
use crate::region::Region;
use crate::session::Session;
use crate::shell::Shell;
use crate::shot::{Screenshot, Shot};
use crate::status::Status;
use crate::wait;
use crate::watcher::{Event, Watcher};

#[derive(Debug, Default)]
pub(crate) struct Machine {
    state: Mutex<State>,
    presence: Mutex<Option<Presence>>,
    watcher: Mutex<Option<Watcher>>,
}

#[derive(Debug, Default)]
struct State {
    stream: Option<FrameStream>,
    session: Option<Session>,
}

impl State {
    fn status(&self) -> Status {
        match &self.session {
            Some(session) => Status::open(session.display(), self.stream.is_some()),
            None => Status::default(),
        }
    }

    fn running(&mut self) -> Result<&mut Session> {
        let session = self.session.as_mut().ok_or(Error::NotRunning)?;
        if !session.is_running() {
            return Err(Error::Stopped);
        }
        Ok(session)
    }

    fn stop(&mut self) {
        self.stream = None;
        self.session = None;
    }
}

impl Machine {
    fn notify(&self, event: Event) {
        let watcher = self.watcher.lock().clone();
        if let Some(watcher) = watcher {
            watcher.notify(event);
        }
    }

    fn with_session<T>(&self, action: impl FnOnce(&mut Session) -> Result<T>) -> Result<T> {
        action(self.state.lock().running()?)
    }

    fn screen(&self) -> Result<Screen> {
        let socket = self.with_session(|session| Ok(session.socket()))?;
        Screen::connect_to(&socket).map_err(capture)
    }

    fn capture_target(&self) -> Result<(PathBuf, Point)> {
        self.with_session(|session| Ok((session.socket(), session.display().capture_size())))
    }

    fn with_input<T>(&self, action: impl FnOnce(&mut InputDriver) -> Result<T>) -> Result<T> {
        self.with_session(|session| action(session.input()))
    }
}

impl Backend for Machine {
    fn kind(&self) -> Kind {
        Kind::Machine
    }

    fn isolated(self: Arc<Self>) -> Arc<dyn Backend> {
        Arc::new(Machine::default())
    }

    fn watch(&self, watcher: Watcher) {
        *self.watcher.lock() = Some(watcher);
    }

    fn status(&self) -> Status {
        self.state.lock().status()
    }

    fn start(&self, display: Display, shell: Shell) -> Result<Status> {
        let mut state = self.state.lock();
        if state.session.as_mut().is_some_and(Session::is_running) {
            return Ok(state.status());
        }

        state.stop();
        *self.presence.lock() = None;
        self.notify(Event::Status(state.status()));

        let session = state.session.insert(Session::start(display, shell)?);
        *self.presence.lock() = Some(session.presence());
        let watcher = self.watcher.lock().clone();
        if let Some(watcher) = watcher {
            state.stream = Some(FrameStream::start(session.socket(), watcher));
        }
        let status = state.status();
        self.notify(Event::Status(status));
        Ok(status)
    }

    fn close(&self) {
        let mut state = self.state.lock();
        state.stop();
        *self.presence.lock() = None;
        self.notify(Event::Status(state.status()));
    }

    fn show(&self, _shown: bool) -> Result<()> {
        Ok(())
    }

    fn screenshot(&self) -> Result<Shot> {
        let (socket, size) = self.capture_target()?;
        Screenshot::Socket(&socket).shot(size)
    }

    fn zoom(&self, region: Region) -> Result<Shot> {
        let (socket, size) = self.capture_target()?;
        Screenshot::Socket(&socket).zoom(size, region)
    }

    fn cursor_position(&self) -> Result<Point> {
        self.with_input(|input| Ok(input.position()))
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
        self.with_session(|session| session.launch(command))
    }

    fn windows(&self) -> Result<Vec<ScreenWindow>> {
        self.with_session(|session| session.windows())
    }

    fn wait_for_window(
        &self,
        matches: &dyn Fn(&ScreenWindow) -> bool,
        timeout: Duration,
    ) -> Result<ScreenWindow> {
        let screen = self.screen()?;
        wait::wait_for_window(screen, matches, timeout)
    }

    fn wait_idle(&self, timeout: Duration) -> Result<()> {
        let screen = self.screen()?;
        wait::wait_idle(screen, &Source::FirstOutput, timeout)
    }

    fn environment(&self) -> Option<Environment> {
        self.presence
            .lock()
            .as_ref()
            .and_then(Presence::environment)
    }
}
