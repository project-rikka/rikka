use rikka_types::Choice;

const FONT_SIZES: [u8; 9] = [14, 15, 16, 17, 18, 20, 22, 25, 28];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Look {
    pub palette: Palette,
    pub typeface: Typeface,
    pub size: FontSize,
    pub width: Width,
    pub spacing: Spacing,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Palette {
    #[default]
    Light,
    Dark,
    Sepia,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Colors {
    pub(crate) background: &'static str,
    pub(crate) text: &'static str,
    pub(crate) secondary: &'static str,
    pub(crate) link: &'static str,
    pub(crate) rule: &'static str,
    pub(crate) surface: &'static str,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Typeface {
    Serif,
    #[default]
    Sans,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FontSize(u8);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Width {
    Narrow,
    #[default]
    Medium,
    Wide,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Spacing {
    Compact,
    #[default]
    Normal,
    Relaxed,
}

impl Palette {
    pub(crate) fn colors(self) -> Colors {
        match self {
            Palette::Light => Colors {
                background: "#ffffff",
                text: "#1d1d1f",
                secondary: "#6b6b70",
                link: "#0b57d0",
                rule: "#dcdce0",
                surface: "#f3f3f5",
            },
            Palette::Dark => Colors {
                background: "#1b1b1d",
                text: "#e4e4e7",
                secondary: "#9d9da3",
                link: "#8ab4f8",
                rule: "#3a3a3f",
                surface: "#27272b",
            },
            Palette::Sepia => Colors {
                background: "#f6efe0",
                text: "#3b3024",
                secondary: "#7a6955",
                link: "#8c4a12",
                rule: "#e0d2b6",
                surface: "#ece2cc",
            },
        }
    }

    pub(crate) fn is_dark(self) -> bool {
        self == Palette::Dark
    }
}

impl FontSize {
    pub const SMALLEST: FontSize = FontSize(0);
    pub const LARGEST: FontSize = FontSize(FONT_SIZES.len() as u8 - 1);

    pub fn step(step: u8) -> FontSize {
        FontSize(step.min(FontSize::LARGEST.0))
    }

    pub fn index(self) -> u8 {
        self.0
    }

    pub fn larger(self) -> FontSize {
        FontSize::step(self.0.saturating_add(1))
    }

    pub fn smaller(self) -> FontSize {
        FontSize(self.0.saturating_sub(1))
    }

    pub fn pixels(self) -> u8 {
        FONT_SIZES[usize::from(self.0)]
    }
}

impl Default for FontSize {
    fn default() -> FontSize {
        FontSize(4)
    }
}

impl Choice for Typeface {
    const ALL: &'static [Typeface] = &[Typeface::Sans, Typeface::Serif];

    fn slug(self) -> &'static str {
        match self {
            Typeface::Sans => "sans",
            Typeface::Serif => "serif",
        }
    }
}

impl Choice for Width {
    const ALL: &'static [Width] = &[Width::Narrow, Width::Medium, Width::Wide];

    fn slug(self) -> &'static str {
        match self {
            Width::Narrow => "narrow",
            Width::Medium => "medium",
            Width::Wide => "wide",
        }
    }
}

impl Choice for Spacing {
    const ALL: &'static [Spacing] = &[Spacing::Compact, Spacing::Normal, Spacing::Relaxed];

    fn slug(self) -> &'static str {
        match self {
            Spacing::Compact => "compact",
            Spacing::Normal => "normal",
            Spacing::Relaxed => "relaxed",
        }
    }
}
