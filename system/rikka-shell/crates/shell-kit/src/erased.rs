use std::time::Duration;

use rikka_theme::Theme;
use rikka_ui::{Handler, State, Ui};

use crate::Placement;
use crate::pointer_mode::PointerMode;
use crate::presence_binding::PresenceBinding;
use crate::settings::SurfaceSettings;
use crate::surface::SurfaceDesc;

type UiSource = Box<dyn Fn(Theme, Placement) -> Ui>;

pub struct ErasedSurface {
    pub settings: SurfaceSettings,
    pub source: UiSource,
    pub moved: Option<Handler<(Placement,)>>,
    pub on_leave: Option<Handler>,
    leaving: Option<u64>,
    leaves: u64,
}

impl ErasedSurface {
    pub(crate) fn new<S: State>(desc: SurfaceDesc<S>) -> Self {
        let SurfaceDesc {
            settings,
            source: factory,
            view,
            start,
            moved,
            leave,
        } = desc;
        Self {
            settings,
            source: Box::new(move |theme, place| {
                let view = view.clone();
                let mut ui = Ui::new(factory(place), move |state: &S| view(state), theme);
                if let Some(start) = &start {
                    ui.start(start.clone());
                }
                ui
            }),
            moved,
            on_leave: leave,
            leaving: None,
            leaves: 0,
        }
    }

    pub fn exit(&self) -> Option<Duration> {
        self.settings
            .presence_binding
            .as_ref()
            .and_then(PresenceBinding::exit)
    }

    pub fn is_up(&self) -> bool {
        self.settings.is_wanted() || self.is_leaving()
    }

    pub fn is_leaving(&self) -> bool {
        self.leaving.is_some()
    }

    pub fn pointer(&self) -> PointerMode {
        match self.is_leaving() {
            true => PointerMode::Ignore,
            false => self.settings.taken(),
        }
    }

    pub fn start_leaving(&mut self) -> u64 {
        self.leaves += 1;
        self.leaving = Some(self.leaves);
        self.leaves
    }

    pub fn stop_leaving(&mut self) {
        self.leaving = None;
    }

    pub fn finish_leaving(&mut self, token: u64) -> bool {
        if self.leaving != Some(token) {
            return false;
        }
        self.leaving = None;
        true
    }
}

impl std::fmt::Debug for ErasedSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ErasedSurface")
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}
