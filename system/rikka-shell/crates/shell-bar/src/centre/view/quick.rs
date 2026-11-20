use rikka_theme::Corner;
use rikka_types::Color;
use rikka_ui::{
    Align, Element, Handler, IntoHandler, Label, Length, MaterialSymbol, Space, Styled, TextStyled,
    View, palette, text, tr,
};
use rikka_widgets::{column, container, icon, row};
use shell_kit::clip;

use crate::centre::detail::Detail;
use crate::centre::state::Centre;

const HEIGHT: f32 = 62.0;

const PADDING: [f32; 2] = [10.0, 8.0];

const ICON: f32 = 20.0;

const CHEVRON: f32 = 18.0;

const TARGET: f32 = 24.0;

const FADED: f32 = 0.38;

#[must_use]
pub(super) fn quick(symbol: MaterialSymbol, title: Label) -> Quick {
    Quick {
        symbol,
        title,
        taskbar_visibility: None,
        on: false,
        here: true,
        toggle: None,
        opens: None,
    }
}

pub(super) struct Quick {
    symbol: MaterialSymbol,
    title: Label,
    taskbar_visibility: Option<Label>,
    on: bool,
    here: bool,
    toggle: Option<Handler>,
    opens: Option<Detail>,
}

impl Quick {
    #[must_use]
    pub(crate) fn taskbar_visibility(mut self, taskbar_visibility: Label) -> Self {
        self.taskbar_visibility = Some(taskbar_visibility);
        self
    }

    #[must_use]
    pub(crate) fn on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }

    #[must_use]
    pub(crate) fn here(mut self, here: bool) -> Self {
        self.here = here;
        self
    }

    #[must_use]
    pub(crate) fn toggle<Marker>(mut self, toggle: impl IntoHandler<(), (), Marker>) -> Self {
        self.toggle = Some(toggle.into_handler());
        self
    }

    #[must_use]
    pub(crate) fn opens(mut self, page: Detail) -> Self {
        self.opens = Some(page);
        self
    }
}

impl View for Quick {
    fn build(self) -> Element {
        let palette = palette();
        let on = self.on && self.here;
        let (background, ink, muted) = match (self.here, on) {
            (false, _) => (
                palette.surface.scale_alpha(FADED),
                palette.text.scale_alpha(FADED),
                palette.text_muted.scale_alpha(FADED),
            ),
            (true, true) => (
                palette.accent,
                palette.on_accent,
                palette.on_accent.scale_alpha(0.72),
            ),
            (true, false) => (palette.surface, palette.text, palette.text_muted),
        };

        let taskbar_visibility = match self.here {
            true => self.taskbar_visibility.unwrap_or_else(|| switched(on)),
            false => tr!("shell.centre.unavailable"),
        };
        let names = column! {
            text(self.title).caption().medium().color(ink).no_wrap();
            text(taskbar_visibility).caption().color(muted).no_wrap()
        };
        let here = self.here;

        let mut tile = container(
            row! {
                icon(self.symbol).extent(Length::px(ICON)).color(ink);
                clip(names);
                if let Some(page) = self.opens.filter(|_| here) {
                    chevron(page, muted);
                }
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width(),
        )
        .background(background)
        .radius(Corner::Panel)
        .padding(PADDING)
        .align_y(Align::Center)
        .height(HEIGHT)
        .fill_width();
        if let Some(toggle) = self.toggle.filter(|_| here) {
            tile = tile.on_press(toggle).focusless();
        }
        tile.build()
    }
}

fn switched(on: bool) -> Label {
    match on {
        true => tr!("shell.centre.on"),
        false => tr!("shell.centre.off"),
    }
}

fn chevron(page: Detail, ink: Color) -> impl View {
    container(
        icon(MaterialSymbol::CHEVRON_RIGHT)
            .extent(Length::px(CHEVRON))
            .color(ink),
    )
    .radius(TARGET / 2.0)
    .width(TARGET)
    .height(TARGET)
    .center()
    .on_press(move |centre: &mut Centre| centre.open(Some(page)))
    .focusless()
}
