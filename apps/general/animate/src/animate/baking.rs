use animate_fluid::FluidSettings;
use animate_inspector::LiquidSettings;
use animate_stage::Notify;
use rikka_app::prelude::*;

use crate::animate::Animate;
use crate::wording;

pub(super) fn notifier(sender: &Sender<Animate>) -> Notify {
    let sender = sender.clone();
    std::sync::Arc::new(move || {
        let _ = sender.send(|animate: &mut Animate| animate.on_stage_notified());
    })
}

impl Animate {
    pub(crate) fn pours(&self) -> bool {
        self.pouring.computing
    }

    pub(crate) fn set_pouring(&mut self, pours: bool) {
        self.pouring.computing = pours;
        self.stage.lock().pause_liquids(!pours);
    }

    pub(crate) fn can_recompute_liquids(&self) -> bool {
        !self.cast.emitters().is_empty()
    }

    pub(crate) fn recompute_liquids(&mut self) {
        self.stage.lock().recompute_liquids();
    }

    pub(crate) fn on_stage_notified(&mut self) {
        let mut stage = self.stage.lock();
        stage.rouse();
        let liquid = stage.liquid_cache();
        let error = stage.take_error();
        stage.watchers().repaint();
        drop(stage);
        self.timeline.set_liquid(liquid);
        match error {
            Some(error) => self.notice = Some(wording::error(&error)),
            None => skip_view(),
        }
    }
}

impl LiquidSettings for Animate {
    fn draft_fluid_settings(&mut self, settings: FluidSettings) {
        self.controls.scene_draft = Some(settings);
    }
    fn fluid_settings(&self) -> FluidSettings {
        self.controls.scene_draft.unwrap_or(self.pouring.settings)
    }
    fn set_fluid_settings(&mut self, settings: FluidSettings) {
        self.controls.scene_draft = None;
        if self.pouring.settings == settings {
            return;
        }
        self.pouring.settings = settings;
        self.preferences.keep_liquid(&settings);
        self.pour_liquids();
    }
}
