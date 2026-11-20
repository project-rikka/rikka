use rikka_appearance::Appearance;
use rikka_settings::seam::Settings;
use rikka_settings::session::appearance;
use rikka_theme::{Palette, Theme};
use rikka_types::Lens;

use crate::policy::{CaptionLook, Glass};

pub(crate) use appearance::DOMAIN;

pub(crate) fn schema() -> rikka_settings::Schema {
    appearance::schema()
}

#[derive(Debug, Default)]
pub(crate) struct GlassStyle {
    fitted: Option<Appearance>,
    lens: Option<Lens>,
    palette: Option<Palette>,
    caption: Option<CaptionLook>,
}

impl GlassStyle {
    pub(crate) fn glass(&mut self, settings: &Settings) -> Glass {
        self.fit(settings);
        Glass::new(blur_radius(settings), self.lens)
    }

    pub(crate) fn palette(&mut self, settings: &Settings) -> Palette {
        self.fit(settings);
        self.palette.unwrap_or_else(|| Theme::default().palette())
    }

    pub(crate) fn caption(&mut self, settings: &Settings) -> CaptionLook {
        self.fit(settings);
        self.caption
            .unwrap_or_else(|| CaptionLook::of(&Theme::default()))
    }

    fn fit(&mut self, settings: &Settings) {
        let appearance = Appearance::read(settings.store());
        if self.fitted.as_ref() == Some(&appearance) {
            return;
        }
        let (theme, lens) = appearance.theme_and_lens();
        self.lens = lens;
        self.palette = Some(theme.palette());
        self.caption = Some(CaptionLook::of(&theme));
        self.fitted = Some(appearance);
    }
}

pub(crate) fn minimizes_windows(settings: &Settings) -> bool {
    settings.read_or(appearance::TASKBAR, appearance::TASKBAR_DEFAULT)
}

fn blur_radius(settings: &Settings) -> f32 {
    let on = settings.read_or(appearance::BLUR, appearance::BLUR_DEFAULT);
    if !on {
        return 0.0;
    }
    let radius = settings.read_or(appearance::BLUR_RADIUS, appearance::BLUR_RADIUS_DEFAULT);
    radius.clamp(0.0, appearance::BLUR_RADIUS_MAX) as f32
}
