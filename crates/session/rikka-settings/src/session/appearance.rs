use rikka_types::Choice;

use crate::key::Key;
use crate::schema::Schema;
use crate::value::Value;

pub const DOMAIN: &str = "rikka.appearance";

pub const THEME: &str = "theme";

pub const WALLPAPER: &str = "wallpaper";

pub const TASKBAR: &str = "taskbar";

pub const TASKBAR_POSITION: &str = "taskbar-position";

pub const TASKBAR_INDICATORS: &str = "taskbar-indicators";

pub const ICON_THEME: &str = "icon-theme";

pub const MATERIAL_SOURCE: &str = "material-source";

pub const MATERIAL_STYLE: &str = "material-style";

pub const MATERIAL_CONTRAST: &str = "material-contrast";

pub const BACKGROUND: &str = "background";

pub const OPACITY: &str = "opacity";

pub const BLUR: &str = "blur";

pub const BLUR_RADIUS: &str = "blur-radius";

pub const UNSET: &str = "";

pub const TASKBAR_DEFAULT: bool = true;

pub const TASKBAR_TOP: &str = "top";

pub const TASKBAR_BOTTOM: &str = "bottom";

pub const TASKBAR_LEFT: &str = "left";

pub const TASKBAR_RIGHT: &str = "right";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TaskbarPosition {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

impl Choice for TaskbarPosition {
    const ALL: &'static [TaskbarPosition] = &[
        TaskbarPosition::Top,
        TaskbarPosition::Bottom,
        TaskbarPosition::Left,
        TaskbarPosition::Right,
    ];

    fn slug(self) -> &'static str {
        match self {
            TaskbarPosition::Top => TASKBAR_TOP,
            TaskbarPosition::Bottom => TASKBAR_BOTTOM,
            TaskbarPosition::Left => TASKBAR_LEFT,
            TaskbarPosition::Right => TASKBAR_RIGHT,
        }
    }
}

pub const INDICATOR_INPUT_METHOD: &str = "input-method";

pub const INDICATOR_AI_USAGE: &str = "ai-usage";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Indicator {
    InputMethod,
    AiUsage,
}

impl Indicator {
    pub const ALL: [Indicator; 2] = [Indicator::InputMethod, Indicator::AiUsage];

    #[must_use]
    pub fn parse(text: &str) -> Option<Indicator> {
        Indicator::ALL
            .into_iter()
            .find(|indicator| indicator.slug() == text)
    }

    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Indicator::InputMethod => INDICATOR_INPUT_METHOD,
            Indicator::AiUsage => INDICATOR_AI_USAGE,
        }
    }

    #[must_use]
    pub fn parse_list<S: AsRef<str>>(names: &[S]) -> Vec<Indicator> {
        Indicator::ALL
            .into_iter()
            .filter(|indicator| {
                names
                    .iter()
                    .any(|slug| Indicator::parse(slug.as_ref()) == Some(*indicator))
            })
            .collect()
    }

    #[must_use]
    pub fn to_value(indicators: impl IntoIterator<Item = Indicator>) -> Value {
        let indicators: Vec<Indicator> = indicators.into_iter().collect();
        Value::List(
            Indicator::ALL
                .into_iter()
                .filter(|indicator| indicators.contains(indicator))
                .map(|indicator| Value::String(indicator.slug().to_owned()))
                .collect(),
        )
    }
}

pub const MATERIAL_KEYS: [&str; 3] = [MATERIAL_SOURCE, MATERIAL_STYLE, MATERIAL_CONTRAST];

pub const MATERIAL_STYLE_DEFAULT: &str = "tonal-spot";

pub const MATERIAL_CONTRAST_DEFAULT: f64 = 0.0;

pub const GLASS_KEYS: [&str; 4] = [BACKGROUND, OPACITY, BLUR, BLUR_RADIUS];

pub const BACKGROUND_SOLID: &str = "solid";

pub const BACKGROUND_GLASS: &str = "glass";

pub const BACKGROUND_LIQUID: &str = "liquid";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Background {
    #[default]
    Solid,
    Glass,
    Liquid,
}

impl Choice for Background {
    const ALL: &'static [Background] = &[Background::Solid, Background::Glass, Background::Liquid];

    fn slug(self) -> &'static str {
        match self {
            Background::Solid => BACKGROUND_SOLID,
            Background::Glass => BACKGROUND_GLASS,
            Background::Liquid => BACKGROUND_LIQUID,
        }
    }
}

pub const OPACITY_DEFAULT: f64 = 1.0;

pub const BLUR_DEFAULT: bool = true;

pub const BLUR_RADIUS_DEFAULT: f64 = 32.0;

pub const BLUR_RADIUS_MAX: f64 = 128.0;

pub const ICON_THEME_DEFAULT: &str = "Adwaita";

#[must_use]
pub fn schema() -> Schema {
    Schema::new(DOMAIN)
        .summary("What the desktop looks like")
        .key(Key::string(THEME, UNSET).summary("The theme's slug, or nothing for the built-in one"))
        .key(
            Key::string(WALLPAPER, UNSET)
                .summary("The name of a wallpaper of the session's library, or nothing for none"),
        )
        .key(
            Key::bool(TASKBAR, TASKBAR_DEFAULT)
                .summary("Whether the taskbar is up on every display"),
        )
        .key(
            Key::choice::<TaskbarPosition>(TASKBAR_POSITION)
                .summary("Which display edge the taskbar stands on: top, bottom, left or right"),
        )
        .key(
            Key::list(
                TASKBAR_INDICATORS,
                Indicator::ALL.map(|indicator| Value::String(indicator.slug().to_owned())),
            )
            .summary("What the taskbar shows beside its status, by indicator name"),
        )
        .key(
            Key::string(MATERIAL_SOURCE, UNSET)
                .summary("The colour a dynamic theme is generated from, as `#rrggbb`"),
        )
        .key(
            Key::string(MATERIAL_STYLE, MATERIAL_STYLE_DEFAULT)
                .summary("Which Material You scheme a dynamic theme is spread into"),
        )
        .key(
            Key::float(MATERIAL_CONTRAST, MATERIAL_CONTRAST_DEFAULT)
                .range(-1.0..=1.0)
                .summary("How far a dynamic theme is pushed from the specified contrast"),
        )
        .key(
            Key::string(ICON_THEME, ICON_THEME_DEFAULT)
                .summary("The icon theme an application's own icon is taken from"),
        )
        .key(
            Key::choice::<Background>(BACKGROUND).summary(
                "What a surface's background is made of: solid colour, glass or liquid glass",
            ),
        )
        .key(
            Key::float(OPACITY, OPACITY_DEFAULT)
                .range(0.0..=1.0)
                .summary("How much of what is behind a surface stays behind it"),
        )
        .key(
            Key::bool(BLUR, BLUR_DEFAULT)
                .summary("Whether what shows through a translucent surface is blurred"),
        )
        .key(
            Key::float(BLUR_RADIUS, BLUR_RADIUS_DEFAULT)
                .range(0.0..=BLUR_RADIUS_MAX)
                .summary("How far that blur reaches, in logical pixels"),
        )
}
