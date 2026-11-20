use rikka_ui::text::{Label, TextColor, TextStyled, text};
use rikka_ui::{Align, Element, Length, Space, Style, Styled, View};

use crate::blocks::container::{Container, container};
use crate::blocks::flex::{column_of, row_of};
use crate::blocks::scrollable::{Scrollable, scrollable};

pub fn page(blocks: impl IntoIterator<Item = impl View>) -> Scrollable {
    scrollable(
        column_of(blocks)
            .gap(Space::ROOMY)
            .padding(Space::ROOMY)
            .fill_width(),
    )
    .fill()
}

pub fn section(heading: impl Into<Label>, content: impl View) -> Container {
    card(
        column_of([text(heading).heading().semibold().build(), content.build()])
            .gap(Space::NORMAL)
            .fill_width(),
    )
}

pub fn card(content: impl View) -> Container {
    container(content)
        .surface()
        .padding(Space::ROOMY)
        .fill_width()
}

#[must_use]
pub struct Field {
    label: Label,
    control: Element,
    color: TextColor,
    style: Style,
}

pub fn field(label: impl Into<Label>, control: impl View) -> Field {
    Field {
        label: label.into(),
        control: control.build(),
        color: TextColor::Muted,
        style: Style {
            width: Some(Length::FILL),
            ..Style::default()
        },
    }
}

impl Field {
    pub fn dimmed(mut self, dimmed: bool) -> Self {
        self.color = match dimmed {
            true => TextColor::Faint,
            false => TextColor::Muted,
        };
        self
    }
}

impl View for Field {
    fn build(self) -> Element {
        row_of([
            text(self.label).color(self.color).no_wrap().build(),
            container(self.control)
                .align_x(Align::End)
                .align_y(Align::Center)
                .grow(1.0)
                .shrink(0.0)
                .build(),
        ])
        .gap(Space::ROOMY)
        .align_items(Align::Center)
        .styled(self.style)
        .build()
    }
}

impl std::fmt::Debug for Field {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Field")
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

impl Styled for Field {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}
