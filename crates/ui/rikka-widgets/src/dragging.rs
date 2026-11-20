use rikka_symbols::MaterialSymbol;
use rikka_theme::Theme;
use rikka_types::{Point, Size};
use rikka_ui::text::TextRun;
use rikka_ui::{Align, Corner, DragIcon, Inset, Styled, TextStyled, View, text, text_width};

use crate::blocks::container::container;
use crate::blocks::flex::row_of;
use crate::media::icon::icon;

const TEXT: f32 = 13.0;
const GLYPH: f32 = 16.0;
const GAP: f32 = 6.0;
const PADDING: f32 = 8.0;
const HEIGHT: f32 = 30.0;
const WIDEST: f32 = 280.0;
const CHARACTERS: usize = 64;
const BESIDE: f32 = 14.0;

pub fn drag_chip(theme: Theme, symbol: MaterialSymbol, label: &str) -> DragIcon {
    let label = shortened(label);
    let run = TextRun::new(label.as_str(), TEXT).no_wrap();
    let label_width = text_width(&run).ceil();
    let width = (PADDING * 2.0 + GLYPH + GAP + label_width).min(WIDEST);
    let size = Size::new(width, HEIGHT);
    let palette = theme.palette();
    DragIcon::new(size, Point::new(-BESIDE, -BESIDE), theme, move || {
        container(
            row_of([
                icon(symbol).extent(GLYPH).color(palette.accent).build(),
                text(label.clone())
                    .text_size(TEXT)
                    .no_wrap()
                    .color(palette.text)
                    .grow(1.0)
                    .min_width(0.0)
                    .build(),
            ])
            .gap(GAP)
            .align_items(Align::Center)
            .fill(),
        )
        .padding(Inset::xy(PADDING, 0.0))
        .background(palette.surface)
        .border(1.0, palette.outline)
        .radius(Corner::Item)
        .fill()
        .build()
    })
}

fn shortened(label: &str) -> String {
    let line = label
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    let line = line.trim();
    let mut preview: String = line.chars().take(CHARACTERS).collect();
    if preview.len() < line.len() || label.trim().lines().nth(1).is_some() {
        preview.push('…');
    }
    preview
}
