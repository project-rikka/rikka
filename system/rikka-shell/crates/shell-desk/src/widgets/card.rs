use rikka_theme::Corner;
use rikka_types::Size as Extent;
use rikka_ui::{Element, Length, Space, Styled, View, palette};
use rikka_widgets::container;

use crate::widgets::Mode;
use crate::widgets::body::body;

pub(crate) const INSET: Space = Space::NORMAL;

pub(super) const INNER: Corner = Corner::Item;

const BORDER: f32 = 1.0;

#[must_use]
pub(crate) fn card(extent: Extent, mode: Mode, view: impl Fn() -> Element + 'static) -> Element {
    let (width, height) = (Length::px(extent.width), Length::px(extent.height));
    container(body(mode == Mode::Shown, view).width(width).height(height))
        .surface()
        .border(BORDER, palette().outline)
        .pane()
        .radius(Corner::Window)
        .clip_content()
        .width(width)
        .height(height)
        .build()
}

#[must_use]
pub(super) fn padded(content: impl View) -> Element {
    container(content).padding(INSET).fill().build()
}
