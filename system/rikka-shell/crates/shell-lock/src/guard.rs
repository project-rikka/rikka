use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use rikka_power::{Action, Power, PowerCapabilities, State as PowerState};
use rikka_types::{Key, Modifiers};
use rikka_ui::{ImageSource, State, after_keyed, skip_view, spawn_blocking, stream};
use shell_kit::{KeyboardState, Need, Period, Placement, WallClock};
use shell_wallpaper::{self as loader, Media};

use crate::gate::Gate;
use crate::who;

const SHAKING: &str = "lock.shaking";
const SHAKE_HELD: Duration = Duration::from_millis(70);

#[derive(Debug)]
pub(crate) struct Sources {
    pub(super) gate: Gate,
    pub(crate) wall: WallClock,
    pub(crate) keyboard: KeyboardState,
    pub(crate) power: Power,
    pub(crate) wallpaper: Option<PathBuf>,
}

#[derive(Debug, State)]
pub(super) struct Guard {
    screen: Placement,
    gate: Gate,
    wall: WallClock,
    keyboard: KeyboardState,
    power: Power,
    wallpaper: Option<PathBuf>,
    media: Option<Media>,
    face: Option<ImageSource>,
    capabilities: PowerCapabilities,
    focus: u64,
    refusals: u64,
    shaking: bool,
    minute: Need,
}

impl Guard {
    #[must_use]
    pub(crate) fn new(screen: Placement, sources: Sources) -> Guard {
        let media = sources
            .wallpaper
            .as_deref()
            .and_then(|file| loader::shared(file, loader::cover_of(&screen)));
        Guard {
            minute: sources.wall.need(Period::Minute),
            refusals: sources.gate.refusals(),
            screen,
            gate: sources.gate,
            wall: sources.wall,
            keyboard: sources.keyboard,
            capabilities: sources.power.state().capabilities,
            power: sources.power,
            wallpaper: sources.wallpaper,
            media,
            face: None,
            focus: 0,
            shaking: false,
        }
    }

    pub(crate) fn start(&mut self) {
        stream(
            self.power.changes(),
            |guard: &mut Guard, state: PowerState| {
                guard.capabilities = state.capabilities;
            },
        );
        stream(self.gate.wakes(), |guard: &mut Guard, ()| guard.on_gate());
        self.load_wallpaper();
        self.load_face();
    }

    fn load_face(&self) {
        let Some(account) = self.gate.account().cloned() else {
            return;
        };
        spawn_blocking(
            move || {
                let face = who::face(&account)?;
                Some(std::fs::read(&face).map_err(|error| (face, error)))
            },
            |guard: &mut Guard, read: Option<Result<Vec<u8>, (PathBuf, std::io::Error)>>| match read
            {
                None => {}
                Some(Ok(bytes)) => guard.face = Some(ImageSource::Bytes(Arc::from(bytes))),
                Some(Err((face, error))) => {
                    tracing::warn!("failed to read {}: {error}", face.display());
                }
            },
        );
    }

    fn load_wallpaper(&self) {
        if self.media.is_some() {
            return;
        }
        let Some(file) = self.wallpaper.clone() else {
            return;
        };
        let cover = loader::cover_of(&self.screen);
        spawn_blocking(
            move || loader::load(&file, cover),
            |guard: &mut Guard, loaded: Result<Media, String>| match loaded {
                Ok(media) => guard.media = Some(media),
                Err(error) => tracing::warn!("lock screen wallpaper unavailable: {error}"),
            },
        );
    }

    #[must_use]
    pub(super) fn gate(&self) -> &Gate {
        &self.gate
    }

    #[must_use]
    pub(crate) fn wall(&self) -> &WallClock {
        &self.wall
    }

    #[must_use]
    pub(crate) fn keyboard(&self) -> &KeyboardState {
        &self.keyboard
    }

    #[must_use]
    pub(crate) fn media(&self) -> Option<&Media> {
        self.media.as_ref().filter(|media| media.is_ready())
    }

    #[must_use]
    pub(crate) fn face(&self) -> Option<&ImageSource> {
        self.face.as_ref()
    }

    #[must_use]
    pub(crate) fn offers(&self, action: Action) -> bool {
        self.capabilities.allows(action)
    }

    #[must_use]
    pub(crate) fn focus(&self) -> u64 {
        self.focus
    }

    #[must_use]
    pub(super) fn is_shaking(&self) -> bool {
        self.shaking
    }

    pub(crate) fn pressed(&mut self) {
        self.focus += 1;
    }

    pub(crate) fn submit(&mut self) {
        skip_view();
        self.gate.submit();
    }

    pub(crate) fn key(&mut self, key: Key, modifiers: Modifiers) -> bool {
        if key != Key::Escape || modifiers != Modifiers::NONE {
            return false;
        }
        skip_view();
        self.gate.clear();
        true
    }

    pub(crate) fn ask(&mut self, action: Action) {
        skip_view();
        self.power.ask(action);
    }

    fn on_gate(&mut self) {
        self.gate.drain();
        let refusals = self.gate.refusals();
        if refusals == self.refusals {
            return skip_view();
        }
        self.refusals = refusals;
        self.shaking = true;
        after_keyed(SHAKING, SHAKE_HELD, |guard: &mut Guard| {
            guard.shaking = false;
        });
    }
}
