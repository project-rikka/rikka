use std::sync::Arc;
use std::time::Duration;

use rikka_capture::ScreenWindow;
use rikka_input::Chord;
use rikka_types::{Modifiers, PointerButton};

use crate::display::Display;
use crate::environment::Environment;
use crate::error::Result;
use crate::kind::Kind;
use crate::region::Region;
use crate::shell::Shell;
use crate::shot::Shot;
use crate::status::Status;
use crate::watcher::Watcher;

pub(crate) type Point = (u32, u32);

pub(crate) trait Backend: std::fmt::Debug + Send + Sync {
    fn kind(&self) -> Kind;

    fn isolated(self: Arc<Self>) -> Arc<dyn Backend>;

    fn watch(&self, watcher: Watcher);

    fn status(&self) -> Status;

    fn start(&self, display: Display, shell: Shell) -> Result<Status>;

    fn close(&self);

    fn show(&self, shown: bool) -> Result<()>;

    fn screenshot(&self) -> Result<Shot>;

    fn zoom(&self, region: Region) -> Result<Shot>;

    fn cursor_position(&self) -> Result<Point>;

    fn move_to(&self, x: u32, y: u32) -> Result<()>;

    fn click(
        &self,
        button: PointerButton,
        count: u32,
        modifiers: Modifiers,
        at: Option<Point>,
    ) -> Result<()>;

    fn press(&self, button: PointerButton, at: Option<Point>) -> Result<()>;

    fn release(&self, button: PointerButton, at: Option<Point>) -> Result<()>;

    fn drag(&self, button: PointerButton, to: Point, modifiers: Modifiers) -> Result<()>;

    fn scroll(&self, dx: f64, dy: f64, modifiers: Modifiers, at: Option<Point>) -> Result<()>;

    fn hold_modifiers(&self, modifiers: Modifiers) -> Result<()>;

    fn key(&self, chord: Chord) -> Result<()>;

    fn hold_key(&self, chord: Chord, duration: Duration) -> Result<()>;

    fn type_text(&self, text: &str) -> Result<()>;

    fn launch(&self, command: &str) -> Result<u32>;

    fn windows(&self) -> Result<Vec<ScreenWindow>>;

    fn wait_for_window(
        &self,
        matches: &dyn Fn(&ScreenWindow) -> bool,
        timeout: Duration,
    ) -> Result<ScreenWindow>;

    fn wait_idle(&self, timeout: Duration) -> Result<()>;

    fn environment(&self) -> Option<Environment>;
}
