use std::sync::Arc;
use std::time::Duration;

use rikka_capture::ScreenWindow;
use rikka_input::Chord;
use rikka_seam::SeamClient;
use rikka_types::{Modifiers, PointerButton};

use crate::backend::Backend;
use crate::desktop::Desktop;
use crate::display::Display;
use crate::environment::Environment;
use crate::error::Result;
use crate::kind::Kind;
use crate::machine::Machine;
use crate::region::Region;
use crate::shell::Shell;
use crate::shot::Shot;
use crate::status::Status;
use crate::watcher::{Event, Watcher};

#[derive(Clone, Debug)]
pub struct Computer {
    backend: Arc<dyn Backend>,
}

impl Default for Computer {
    fn default() -> Computer {
        Computer::machine()
    }
}

impl Computer {
    #[must_use]
    pub fn machine() -> Computer {
        Computer {
            backend: Arc::new(Machine::default()),
        }
    }

    #[must_use]
    pub fn desktop(seam: SeamClient) -> Computer {
        Computer {
            backend: Arc::new(Desktop::new(seam)),
        }
    }

    #[must_use]
    pub fn kind(&self) -> Kind {
        self.backend.kind()
    }

    #[must_use]
    pub fn isolated(&self) -> Computer {
        Computer {
            backend: Arc::clone(&self.backend).isolated(),
        }
    }

    pub fn watch(&self, on_event: impl Fn(Event) + Send + Sync + 'static) {
        self.backend.watch(Watcher::new(Arc::new(on_event)));
    }

    #[must_use]
    pub fn status(&self) -> Status {
        self.backend.status()
    }

    pub fn start(&self, display: Display, shell: Shell) -> Result<Status> {
        self.backend.start(display, shell)
    }

    pub fn close(&self) {
        self.backend.close();
    }

    pub fn show(&self, visible: bool) -> Result<()> {
        self.backend.show(visible)
    }

    pub fn screenshot(&self) -> Result<Shot> {
        self.backend.screenshot()
    }

    pub fn zoom(&self, region: Region) -> Result<Shot> {
        self.backend.zoom(region)
    }

    pub fn cursor_position(&self) -> Result<(u32, u32)> {
        self.backend.cursor_position()
    }

    pub fn move_to(&self, x: u32, y: u32) -> Result<()> {
        self.backend.move_to(x, y)
    }

    pub fn click(
        &self,
        button: PointerButton,
        count: u32,
        modifiers: Modifiers,
        at: Option<(u32, u32)>,
    ) -> Result<()> {
        self.backend.click(button, count, modifiers, at)
    }

    pub fn press(&self, button: PointerButton, at: Option<(u32, u32)>) -> Result<()> {
        self.backend.press(button, at)
    }

    pub fn release(&self, button: PointerButton, at: Option<(u32, u32)>) -> Result<()> {
        self.backend.release(button, at)
    }

    pub fn drag(&self, button: PointerButton, to: (u32, u32), modifiers: Modifiers) -> Result<()> {
        self.backend.drag(button, to, modifiers)
    }

    pub fn scroll(
        &self,
        dx: f64,
        dy: f64,
        modifiers: Modifiers,
        at: Option<(u32, u32)>,
    ) -> Result<()> {
        self.backend.scroll(dx, dy, modifiers, at)
    }

    pub fn hold_modifiers(&self, modifiers: Modifiers) -> Result<()> {
        self.backend.hold_modifiers(modifiers)
    }

    pub fn key(&self, chord: Chord) -> Result<()> {
        self.backend.key(chord)
    }

    pub fn hold_key(&self, chord: Chord, duration: Duration) -> Result<()> {
        self.backend.hold_key(chord, duration)
    }

    pub fn type_text(&self, text: &str) -> Result<()> {
        self.backend.type_text(text)
    }

    pub fn launch(&self, command: &str) -> Result<u32> {
        self.backend.launch(command)
    }

    pub fn windows(&self) -> Result<Vec<ScreenWindow>> {
        self.backend.windows()
    }

    pub fn wait_for_window(
        &self,
        matches: impl Fn(&ScreenWindow) -> bool,
        timeout: Duration,
    ) -> Result<ScreenWindow> {
        self.backend.wait_for_window(&matches, timeout)
    }

    pub fn wait_idle(&self, timeout: Duration) -> Result<()> {
        self.backend.wait_idle(timeout)
    }

    #[must_use]
    pub fn environment(&self) -> Option<Environment> {
        self.backend.environment()
    }
}
