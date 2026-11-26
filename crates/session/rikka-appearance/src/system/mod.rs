mod colors;
mod contrast;
mod messages;
mod registry;
mod watch;

use rikka_settings::session::appearance::{self, Background};
use rikka_theme::winui::{self, AccentPalette, SystemColors};
use rikka_theme::{MaterialYou, Result, Theme, ThemeRegistry, Translucency, Variant};
use windows::UI::ViewManagement::UISettings;

use crate::dress;

pub use messages::is_appearance_change;
pub use watch::AppearanceWatch;

#[derive(Clone, Debug, PartialEq)]
#[must_use]
pub struct Appearance {
    scheme: Scheme,
    chosen: Chosen,
}

#[derive(Clone, Debug, PartialEq)]
enum Scheme {
    Fluent {
        variant: Variant,
        accent: AccentPalette,
    },
    HighContrast(SystemColors),
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Chosen {
    slug: Option<String>,
    background: Option<Background>,
}

impl Appearance {
    pub fn current() -> Appearance {
        Appearance {
            scheme: Scheme::current(),
            chosen: Chosen::default(),
        }
    }

    pub fn theme(&self) -> Theme {
        match (&self.scheme, &self.chosen) {
            (Scheme::HighContrast(colors), _) => winui::high_contrast(colors),
            (
                Scheme::Fluent { variant, accent },
                Chosen {
                    slug: None,
                    background,
                },
            ) => match background {
                None => winui::theme(*variant, accent),
                Some(background) => {
                    worn(winui::theme(*variant, accent), *background).with_translucency(glaze())
                }
            },
            (
                Scheme::Fluent { .. },
                Chosen {
                    slug: Some(slug),
                    background,
                },
            ) => {
                let spec = ThemeRegistry::named(slug, self.material());
                Theme::from_spec(&dress::dressed(spec, background.unwrap_or_default()))
                    .with_translucency(glaze())
            }
        }
    }

    #[must_use]
    pub fn is_dark(&self) -> bool {
        match &self.scheme {
            Scheme::Fluent { variant, .. } => variant.is_dark(),
            Scheme::HighContrast(colors) => colors.variant().is_dark(),
        }
    }

    pub fn material(&self) -> MaterialYou {
        match &self.scheme {
            Scheme::Fluent { accent, .. } => MaterialYou::new(accent.accent),
            Scheme::HighContrast(_) => MaterialYou::default(),
        }
    }

    pub fn registry(&self) -> Result<ThemeRegistry> {
        ThemeRegistry::discover(self.material())
    }

    pub fn with_theme(mut self, slug: &str) -> Appearance {
        self.chosen.slug = Some(slug.to_owned());
        self
    }

    pub fn with_background(mut self, background: Background) -> Appearance {
        self.chosen.background = Some(background);
        self
    }
}

impl Scheme {
    fn current() -> Scheme {
        if let Some(colors) = contrast::high_contrast() {
            return Scheme::HighContrast(colors);
        }
        let settings = UISettings::new()
            .inspect_err(|error| tracing::warn!("failed to open the system ui settings: {error}"))
            .ok();
        Scheme::Fluent {
            variant: colors::variant(settings.as_ref()),
            accent: colors::accent(settings.as_ref()),
        }
    }
}

fn worn(theme: Theme, background: Background) -> Theme {
    match background {
        Background::Solid => theme,
        Background::Glass => theme.glass(),
        Background::Liquid => theme.liquid(),
    }
}

fn glaze() -> Translucency {
    dress::glaze(
        appearance::OPACITY_DEFAULT,
        appearance::BLUR_DEFAULT,
        appearance::BLUR_RADIUS_DEFAULT,
    )
}
