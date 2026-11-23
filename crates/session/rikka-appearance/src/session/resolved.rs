use rikka_seam::Delivery;
use rikka_settings::seam::Settings;
use rikka_settings::seam::protocol;
use rikka_settings::session::appearance;

use rikka_theme::Theme;

use crate::session::appearance::Appearance;

#[derive(Debug)]
pub struct ResolvedAppearance {
    appearance: Appearance,
}

impl ResolvedAppearance {
    #[must_use]
    pub fn new(appearance: Appearance) -> Self {
        Self { appearance }
    }

    #[must_use]
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            appearance: Appearance::read(settings.store()),
        }
    }

    pub fn theme(&self) -> Theme {
        self.appearance.theme()
    }

    #[must_use]
    pub fn appearance(&self) -> &Appearance {
        &self.appearance
    }

    pub fn refresh(&mut self, settings: &Settings) -> Option<Theme> {
        self.appearance.update(settings.store())
    }

    pub fn apply_delivery(
        &mut self,
        settings: &mut Settings,
        delivery: &Delivery,
    ) -> Option<Theme> {
        match delivery {
            Delivery::Event(event) => {
                let changed = protocol::changed(event, appearance::DOMAIN)?;
                settings.apply_change(&changed);
            }
            Delivery::Lagged(_) => settings.refresh().ok()?,
        }
        self.refresh(settings)
    }
}
