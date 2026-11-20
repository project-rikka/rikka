use rikka_types::Color;
use rikka_ui::{Element, Inset, Label, Space, Styled, TextStyled, View, text};
use rikka_widgets::{column, column_of, row, spacer};

pub(crate) const LABEL: f32 = 92.0;
pub(crate) const FIELD: f32 = 76.0;

pub fn form_sheet(rows: Vec<Element>) -> Element {
    column_of(rows).gap(Space::NORMAL).fill_width().any()
}

pub fn labelled(label: Label, control: impl View) -> Element {
    row! {
        text(label).caption().muted().width(LABEL).shrink(0.0);
        rikka_widgets::container(control).grow(1.0);
    }
    .centered()
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

pub fn note(message: Label) -> Element {
    text(message).caption().muted().fill_width().any()
}

pub(crate) fn heading(label: Label) -> Element {
    text(label).bold().fill_width().any()
}

pub(crate) fn captioned(label: Label, control: Element) -> Element {
    column! {
        text(label).caption().muted();
        control;
    }
    .gap(Space::TIGHT)
    .fill_width()
    .any()
}

pub(crate) fn ends((low, high): (Label, Label), control: Element) -> Element {
    column! {
        row! {
            text(low).caption().muted();
            spacer().grow(1.0);
            text(high).caption().muted();
        }
        .margin(Inset::exact(0.0, FIELD + Space::SNUG.px(), 0.0, 0.0))
        .fill_width();
        control;
    }
    .gap(Space::TIGHT)
    .fill_width()
    .any()
}

pub(crate) fn color_name(color: Color) -> Label {
    Label::from(color.with_alpha(1.0).to_hex().to_ascii_uppercase())
}
