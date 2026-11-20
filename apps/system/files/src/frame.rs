use files_core::Location;
use rikka_app::prelude::*;
use rikka_ui::{Element, IntoHandler, View};
use rikka_widgets::column;

use crate::browser::Browser;
use crate::crumbs::Crumbs;
use crate::places::Places;
use crate::tab::Tab;

pub(crate) const MARK: &str = "files-mark";
const STAGE: &str = "files-stage";
pub(crate) const PLACES_MARK: &str = "files-places-mark";
pub(crate) const SIDEBAR: f32 = 208.0;
pub(crate) const PATH_MIN: f32 = 96.0;

pub(crate) fn places(list: Places) -> impl View {
    let height = list.height();
    row! {
        container(
            scrollable(
                row! {
                    spacer().width(0.0).height(height).build().keyed(PLACES_MARK);
                    list.build().grow(1.0).height(height);
                }
                .fill_width(),
            )
            .fill_width()
            .fill_height(),
        )
        .padding(Inset::xy(8.0, 10.0))
        .width(SIDEBAR)
        .fill_height();
        rule_upright();
    }
}

pub(crate) fn listing(tab: &Tab, at: usize, browser: Browser) -> impl View {
    row! {
        spacer().width(0.0).fill_height().build().keyed(MARK);
        page_stack((at, tab.history().at()), browser.build().keyed(tab.id()))
            .named((tab.id(), tab.history().at()))
            .grow(1.0)
            .fill_height();
    }
    .grow(1.0)
    .fill_height()
}

pub(crate) fn sheet<M>(
    width: f32,
    key: impl std::hash::Hash + 'static,
    body: impl View,
    dismiss: impl IntoHandler<(), (), M>,
) -> Vec<Element> {
    rikka_widgets::sheet(column! { body; }.fill_width())
        .width(width)
        .keyed(key)
        .centered_on(STAGE)
        .on_dismiss(dismiss)
        .with_scrim()
}

pub(crate) fn stage(body: impl View) -> Element {
    body.build().keyed(STAGE)
}

#[derive(Debug)]
pub(crate) struct Shown<T> {
    showing: Option<T>,
    key: u64,
}

impl<T> Default for Shown<T> {
    fn default() -> Self {
        Self {
            showing: None,
            key: 0,
        }
    }
}

impl<T> Shown<T> {
    pub(crate) fn show(&mut self, value: T) {
        self.key += 1;
        self.showing = Some(value);
    }

    pub(crate) fn dismiss(&mut self) {
        self.showing = None;
    }

    pub(crate) fn showing(&self) -> Option<&T> {
        self.showing.as_ref()
    }

    pub(crate) fn showing_mut(&mut self) -> Option<&mut T> {
        self.showing.as_mut()
    }

    pub(crate) fn key(&self) -> u64 {
        self.key
    }
}

pub(crate) fn path_bar<G, E, B, S>(
    at: &Location,
    typed: Option<&TextDocument>,
    go: impl IntoHandler<(Location,), (), G>,
    edit: impl IntoHandler<(), (), E>,
    blur: impl IntoHandler<(), (), B>,
    submit: impl IntoHandler<(), (), S>,
) -> Element {
    let Some(typed) = typed else {
        return Crumbs::new(at)
            .on_go(go)
            .on_edit(edit)
            .build()
            .grow(1.0)
            .min_width(PATH_MIN);
    };
    text_field(typed)
        .placeholder(tr!("files.toolbar.path"))
        .autofocus(true)
        .on_submit(submit)
        .on_blur(blur)
        .grow(1.0)
        .min_width(PATH_MIN)
        .build()
}

pub(crate) fn step<M>(
    symbol: MaterialSymbol,
    command: Command,
    press: impl IntoHandler<(), (), M>,
    enabled: bool,
) -> ToolButton {
    tool_button(symbol, command_label(command))
        .shortcut(command.chord())
        .control_size(ControlSize::Medium)
        .enabled(enabled)
        .on_press(press)
}
