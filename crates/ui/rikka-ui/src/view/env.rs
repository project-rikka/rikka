use rikka_theme::Theme;
use rikka_types::Color;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Env {
    pub text_color: Color,
    pub font_size: f32,
    pub carried: bool,
    pub leaving: bool,
}

impl Env {
    pub(crate) fn from_theme(theme: &Theme) -> Self {
        Self {
            text_color: theme.palette().text,
            font_size: theme.typography().body_size,
            carried: false,
            leaving: false,
        }
    }
}
