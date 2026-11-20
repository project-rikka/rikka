use rikka_text::Family;
use rikka_theme::{Corner, Theme};

#[derive(Clone, Debug)]
pub(crate) struct MarkdownStyle {
    pub(crate) text_size: f32,
    pub(crate) line_height: f32,
    pub(crate) family: Family,
    pub(crate) code_family: Family,
    heading_scale: [f32; 6],
    pub(crate) block_gap: f32,
    pub(crate) list_indent: f32,
    pub(crate) quote_bar: f32,
    pub(crate) padding: f32,
    pub(crate) radius: f32,
    pub(crate) rule: f32,
}

impl MarkdownStyle {
    pub(crate) fn from_theme(theme: &Theme) -> Self {
        let text_size = theme.typography().body_size;
        Self {
            text_size,
            line_height: (text_size * 1.6).round(),
            family: Family::SansSerif,
            code_family: rikka_ui::Family::parse("monospace"),
            heading_scale: [2.0, 1.55, 1.3, 1.12, 1.0, 0.9],
            block_gap: (text_size * 0.9).round(),
            list_indent: (text_size * 1.6).round(),
            quote_bar: 3.0,
            padding: (text_size * 0.75).round(),
            radius: Corner::Item.radius(),
            rule: 1.0,
        }
    }

    pub(crate) fn heading_size(&self, level: usize) -> f32 {
        let scale = self
            .heading_scale
            .get(level.saturating_sub(1))
            .copied()
            .unwrap_or(1.0);
        (self.text_size * scale).round()
    }

    pub(crate) fn code_size(&self) -> f32 {
        (self.text_size * 0.9).round()
    }
}
