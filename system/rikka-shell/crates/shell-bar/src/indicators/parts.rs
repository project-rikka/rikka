use rikka_types::Rect;
use rikka_ui::{
    Align, Inset, Label, Length, MaterialSymbol, Space, Styled, TextStyled, View, palette, text,
};
use rikka_widgets::{column, container, icon, row, spacer};
use shell_kit::highlight_button;

use crate::dropdown_panel::{DETAIL, ROW_DEPTH, ROW_INSET, ROW_RADIUS};

const INDENT: f32 = 16.0;
const SYMBOL: f32 = 18.0;

#[must_use]
pub(crate) fn heading(title: impl Into<Label>) -> impl View {
    container(text(title).caption().muted().no_wrap()).padding(Inset::edges(
        Space::TIGHT,
        ROW_INSET,
        Space::HAIR,
        ROW_INSET,
    ))
}

#[must_use]
pub(crate) fn rule() -> impl View {
    rikka_widgets::rule().margin(Inset::xy(ROW_INSET, Space::TIGHT))
}

#[must_use]
pub(crate) fn entry(label: impl Into<Label>, press: impl Fn() + 'static) -> Entry {
    Entry {
        label: label.into(),
        detail: None,
        checked: false,
        trailing: None,
        depth: 0,
        press: Box::new(press),
    }
}

pub(crate) struct Entry {
    label: Label,
    detail: Option<Label>,
    checked: bool,
    trailing: Option<MaterialSymbol>,
    depth: usize,
    press: Box<dyn Fn()>,
}

impl Entry {
    #[must_use]
    pub(crate) fn detail(mut self, detail: impl Into<Label>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    #[must_use]
    pub(crate) fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    #[must_use]
    pub(crate) fn trailing(mut self, symbol: MaterialSymbol) -> Self {
        self.trailing = Some(symbol);
        self
    }

    #[must_use]
    pub(crate) fn depth(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }
}

impl View for Entry {
    fn build(self) -> rikka_ui::Element {
        let palette = palette();
        let lines = column! {
            text(self.label).label().no_wrap();
            if let Some(detail) = self.detail {
                text(detail).text_size(DETAIL).muted().no_wrap();
            }
        }
        .gap(Space::HAIR)
        .grow(1.0);
        let check = icon(MaterialSymbol::CHECK)
            .extent(Length::px(SYMBOL))
            .color(palette.accent)
            .opacity(if self.checked { 1.0 } else { 0.0 });
        let body = row! {
            spacer().width(INDENT * self.depth as f32).grow(0.0);
            check;
            lines;
            self.trailing.map(|symbol| {
                icon(symbol).extent(Length::px(SYMBOL)).color(palette.text_muted)
            });
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width()
        .padding(Inset::xy(ROW_INSET, ROW_DEPTH));
        let press = self.press;
        highlight_button(body, move |_: Rect| press())
            .wide()
            .radius(ROW_RADIUS)
            .on_background(palette.surface)
            .build()
    }
}
