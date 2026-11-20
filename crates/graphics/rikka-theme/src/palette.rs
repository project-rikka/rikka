use rikka_types::Color;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub background: Color,
    pub surface: Color,
    pub surface_variant: Color,
    pub text: Color,
    pub text_muted: Color,
    pub accent: Color,
    pub on_accent: Color,
    pub selection: Color,
    pub on_selection: Color,
    pub danger: Color,
    pub on_danger: Color,
    pub warning: Color,
    pub on_warning: Color,
    pub success: Color,
    pub on_success: Color,
    pub outline: Color,
    pub overlay: Color,
}

impl Palette {
    #[must_use]
    pub fn ink(self, selected: bool) -> Color {
        if selected {
            self.on_selection
        } else {
            self.text
        }
    }
}
