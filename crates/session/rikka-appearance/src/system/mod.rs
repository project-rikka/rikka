mod colors;
mod contrast;
mod messages;
mod registry;

use rikka_theme::winui::{self, AccentPalette, SystemColors};
use rikka_theme::{Theme, Variant};
use windows::UI::ViewManagement::UISettings;

pub use messages::is_appearance_change;

#[derive(Clone, Debug, PartialEq)]
#[must_use]
pub struct Appearance {
    scheme: Scheme,
}

#[derive(Clone, Debug, PartialEq)]
enum Scheme {
    Fluent {
        variant: Variant,
        accent: AccentPalette,
    },
    HighContrast(SystemColors),
}

impl Appearance {
    pub fn current() -> Appearance {
        if let Some(colors) = contrast::high_contrast() {
            return Appearance {
                scheme: Scheme::HighContrast(colors),
            };
        }
        let settings = UISettings::new()
            .inspect_err(|error| tracing::warn!("failed to open the system ui settings: {error}"))
            .ok();
        Appearance {
            scheme: Scheme::Fluent {
                variant: colors::variant(settings.as_ref()),
                accent: colors::accent(settings.as_ref()),
            },
        }
    }

    pub fn theme(&self) -> Theme {
        match &self.scheme {
            Scheme::Fluent { variant, accent } => winui::theme(*variant, accent),
            Scheme::HighContrast(colors) => winui::high_contrast(colors),
        }
    }

    #[must_use]
    pub fn is_dark(&self) -> bool {
        match &self.scheme {
            Scheme::Fluent { variant, .. } => variant.is_dark(),
            Scheme::HighContrast(colors) => colors.variant().is_dark(),
        }
    }
}
