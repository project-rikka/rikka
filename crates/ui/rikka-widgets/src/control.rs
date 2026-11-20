use rikka_theme::{Corner, Theme};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ControlSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl ControlSize {
    pub const fn height(self) -> f32 {
        match self {
            Self::Small => 30.0,
            Self::Medium => 36.0,
            Self::Large => 44.0,
        }
    }

    pub fn padding_x(self) -> f32 {
        match self {
            Self::Small => 12.0,
            Self::Medium => 16.0,
            Self::Large => 20.0,
        }
    }

    pub fn text_inset(self) -> f32 {
        self.padding_x() * 0.75
    }

    pub fn gap(self) -> f32 {
        match self {
            Self::Small => 6.0,
            Self::Medium => 8.0,
            Self::Large => 10.0,
        }
    }

    pub fn icon_size(self) -> f32 {
        match self {
            Self::Small => 16.0,
            Self::Medium => 18.0,
            Self::Large => 20.0,
        }
    }

    pub fn indicator(self) -> f32 {
        match self {
            Self::Small => 18.0,
            Self::Medium => 20.0,
            Self::Large => 24.0,
        }
    }

    pub fn font_size(self, theme: &Theme) -> f32 {
        let scale = match self {
            Self::Small => 0.88,
            Self::Medium => 1.0,
            Self::Large => 1.08,
        };
        theme.typography().body_size * scale
    }

    pub const fn radius(self) -> f32 {
        match self {
            Self::Small => Corner::Item.radius(),
            Self::Medium | Self::Large => Corner::Control.radius(),
        }
    }
}
