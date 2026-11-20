use rikka_app::prelude::*;
use rikka_settings::session::appearance;
use rikka_theme::{MaterialStyle, MaterialYou, ThemeRegistry};
use rikka_widgets::column;

use crate::amount::amount;
use crate::appearance::glass::glass;
use crate::appearance::themes::{self, Installed, ThemeChoice};
use crate::appearance::wallpapers::{self, Wallpapers};
use crate::controls::flag_or;
use crate::domain::{Domain, domain_page};
use crate::notice::domain_notice;
use crate::page::Page;
use crate::pending::Pending;
use crate::picker::picker;
use crate::session::Session;

const THEME: &str = "theme";

const INDICATORS: [(appearance::Indicator, &str); 2] = [
    (
        appearance::Indicator::InputMethod,
        "settings.appearance.indicators.input-method",
    ),
    (
        appearance::Indicator::AiUsage,
        "settings.appearance.indicators.ai-usage",
    ),
];

const CONTRAST: std::ops::RangeInclusive<f32> = -1.0..=1.0;
const CONTRAST_STEP: f32 = 0.1;

#[derive(State)]
pub(crate) struct Appearance {
    themes: Vec<Installed>,
    theme_menu_open: bool,
    domain: Domain,
    wallpapers: Wallpapers,
}

impl Page for Appearance {
    fn opened(&mut self, _: &Pending, session: &Session) {
        self.domain.open::<Self>(session);
        self.wallpapers.watch();
    }

    fn closed(&mut self) {
        self.wallpapers.conceal();
    }
}

domain_page!(Appearance);

impl Appearance {
    pub(crate) fn new(installed: Option<&ThemeRegistry>) -> Self {
        Self {
            themes: themes::installed(installed),
            theme_menu_open: false,
            domain: Domain::new(appearance::DOMAIN),
            wallpapers: Wallpapers::new(),
        }
    }

    pub(crate) fn reveal_wallpapers(&mut self) {
        self.wallpapers.reveal();
    }

    pub(super) fn set_theme_menu_open(&mut self, open: bool) {
        self.theme_menu_open = open;
    }

    pub(super) fn pick(&mut self, pick: ThemeChoice) {
        match pick {
            ThemeChoice::Default => {
                self.theme_menu_open = false;
                self.domain.reset(THEME);
            }
            ThemeChoice::Category => self.theme_menu_open = true,
            ThemeChoice::Theme(index) => {
                self.theme_menu_open = false;
                if let Some(slug) = self.themes.get(index).map(|theme| theme.slug.clone()) {
                    self.domain.set(THEME, &slug);
                }
            }
        }
    }

    fn theme_index(&self) -> Option<usize> {
        let slug = self.domain.non_empty_string(THEME)?;
        self.themes.iter().position(|theme| theme.slug == slug)
    }

    fn showing(&self) -> Option<String> {
        self.domain.non_empty_string(appearance::WALLPAPER)
    }

    fn taskbar(&self) -> bool {
        self.domain
            .get(appearance::TASKBAR)
            .unwrap_or(appearance::TASKBAR_DEFAULT)
    }

    fn indicators(&self) -> Vec<appearance::Indicator> {
        self.domain
            .get::<Vec<String>>(appearance::TASKBAR_INDICATORS)
            .map_or_else(
                || appearance::Indicator::ALL.to_vec(),
                |value| appearance::Indicator::parse_list(&value),
            )
    }

    fn show_indicator(&mut self, indicator: appearance::Indicator, shown: bool) {
        let remaining = self
            .indicators()
            .into_iter()
            .filter(|shown| *shown != indicator)
            .chain(shown.then_some(indicator));
        self.domain.set_value(
            appearance::TASKBAR_INDICATORS,
            appearance::Indicator::to_value(remaining),
        );
    }

    fn style(&self) -> MaterialStyle {
        self.domain
            .non_empty_string(appearance::MATERIAL_STYLE)
            .and_then(|slug| MaterialStyle::from_slug(&slug))
            .unwrap_or_default()
    }

    fn contrast(&self) -> f32 {
        self.domain
            .get(appearance::MATERIAL_CONTRAST)
            .unwrap_or(appearance::MATERIAL_CONTRAST_DEFAULT) as f32
    }
}

pub(crate) fn view(appearance: &Appearance) -> impl View {
    column! {
        domain_notice(
            &appearance.domain,
            "settings.appearance.unavailable",
            "settings.appearance.refused",
        );
        section(tr!("settings.appearance.desktop"), desktop(appearance));
        section(
            tr!("settings.appearance.indicators.title"),
            indicators(appearance),
        );
        glass(appearance);
        material(appearance);
        wallpaper_section(appearance);
        if appearance.theme_menu_open {
            themes::panel(&appearance.themes, appearance.theme_index());
        }
    }
    .gap(Space::ROOMY)
}

fn wallpaper_section(appearance: &Appearance) -> Element {
    appearance.wallpapers.section(section(
        tr!("settings.appearance.wallpaper"),
        wallpapers::view(&appearance.wallpapers, appearance.showing().as_deref()),
    ))
}

fn desktop(appearance: &Appearance) -> impl View {
    column! {
        field(
            tr!("settings.appearance.theme"),
            themes::trigger(&appearance.themes, appearance.theme_index(), appearance.theme_menu_open),
        );
        flag_or(
            appearance,
            tr!("settings.appearance.taskbar"),
            appearance::TASKBAR,
            appearance::TASKBAR_DEFAULT,
        );
    }
    .gap(Space::SNUG)
}

fn indicators(appearance: &Appearance) -> impl View {
    let dimmed = !appearance.taskbar();
    let shown = appearance.indicators();
    column! {
        for (indicator, label) in INDICATORS {
            field(
                Label::translated(label),
                switch(shown.contains(&indicator)).on_toggle(
                    move |appearance: &mut Appearance, on: bool| {
                        appearance.show_indicator(indicator, on);
                    },
                ),
            )
            .dimmed(dimmed);
        }
    }
    .gap(Space::SNUG)
}

fn material(appearance: &Appearance) -> Option<impl View> {
    if !appearance
        .domain
        .non_empty_string(THEME)
        .is_some_and(|slug| MaterialYou::names(&slug))
    {
        return None;
    }
    let contrast = appearance.contrast();
    Some(section(
        tr!("settings.appearance.material.title"),
        column! {
            field(tr!("settings.appearance.material.style"), styles(appearance.style()));
            field(
                tr!("settings.appearance.material.contrast"),
                amount(
                    contrast,
                    CONTRAST,
                    CONTRAST_STEP,
                    format!("{:.0}%", contrast * 100.0),
                    |appearance: &mut Appearance, contrast: f32| {
                        appearance
                            .domain
                            .set(appearance::MATERIAL_CONTRAST, f64::from(contrast));
                    },
                ),
            );
        }
        .gap(Space::SNUG),
    ))
}

fn styles(selected: MaterialStyle) -> impl View {
    picker(
        MaterialStyle::ALL
            .iter()
            .position(|style| *style == selected),
        MaterialStyle::ALL.map(|style| {
            Label::translated(&format!("settings.appearance.material.{}", style.slug()))
        }),
        move |appearance: &mut Appearance, index: usize| {
            let style = MaterialStyle::ALL.get(index).copied().unwrap_or(selected);
            appearance
                .domain
                .set(appearance::MATERIAL_STYLE, style.slug());
        },
    )
}

impl std::fmt::Debug for Appearance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Appearance")
            .field("themes", &self.themes.len())
            .field("wallpapers", &self.wallpapers.count())
            .finish_non_exhaustive()
    }
}
