use rikka_app::prelude::*;
use rikka_web::{NavigationButton, web_view};

use crate::model::Tab;
use crate::page::Page;
use crate::requests::TabRequests;
use crate::shell::Shell;
use crate::{gestures, internal, keys, peek};
use browser_model::TabId;

const PROGRESS: f32 = 2.0;

pub(crate) fn view(shell: &Shell) -> Element {
    let model = shell.model();
    let Some(tab_entry) = model.active(shell.id()) else {
        return empty();
    };
    let page = match model.shown_split(shell.id()) {
        Some(group) => crate::split::view(shell, group),
        None => crate::split::alone(shell, tab_entry),
    };
    drop(model);
    layered(page, window_overlays(shell))
}

pub(crate) fn column() -> Element {
    let page = column_of([
        component(|shell: &Shell| shell, view).fill(),
        component(|shell: &Shell| shell, progress),
    ])
    .fill();
    column_of([
        component(|shell: &Shell| shell, crate::default_browser::prompt_view),
        page.any(),
    ])
    .fill()
    .any()
}

pub(crate) fn tab_page(shell: &Shell, tab_entry: &Tab) -> Element {
    layered(body(shell, tab_entry), tab_overlays(shell, tab_entry.id))
}

fn tab_overlays(shell: &Shell, tab: TabId) -> Vec<Element> {
    [
        gestures::indicator(shell, tab),
        crate::split::landing_mark(shell, tab),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn window_overlays(shell: &Shell) -> Vec<Element> {
    [
        peek::view(shell),
        crate::extensions::popup(shell),
        crate::extensions::sheet(shell, shell.id()),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn layered(base: Element, overlays: Vec<Element>) -> Element {
    let overlays = overlays
        .into_iter()
        .map(|overlay| overlay.position(0.0, 0.0));
    column_of(std::iter::once(base).chain(overlays))
        .fill()
        .any()
}

fn body(shell: &Shell, tab_entry: &Tab) -> Element {
    if let Some(internal) = tab_entry.shown_internal() {
        return internal::view(&internal, tab_entry.id).keyed(("internal", tab_entry.id));
    }
    let active = shell
        .model()
        .window(shell.id())
        .and_then(|layout| layout.active());
    if active == Some(tab_entry.id)
        && let Some(failure) = shell.engine_error()
    {
        return status_page(MaterialSymbol::WARNING, failure.to_owned())
            .tint(palette().danger)
            .any();
    }
    if let Some(reading) = crate::reader::view(shell, tab_entry.id) {
        return reading;
    }
    let loading = *tab_entry.meta.loading;
    let tab = tab_entry.id;
    match tab_entry.page() {
        Page::Live(live) => {
            let holds_focus = shell
                .browser()
                .page_services()
                .requests()
                .get(tab)
                .is_some_and(TabRequests::holds_page_focus);
            let surface = web_view(&live.view)
                .reserve_shortcuts(keys::reserved(loading).chain(peek::reserved(shell, tab)))
                .retain_focus(holds_focus)
                .on_navigation_button(|shell: &mut Shell, button: NavigationButton| {
                    shell.navigation_button(button)
                })
                .fill();
            let caught = gestures::catch_gestures(
                crate::split::focusing(surface, tab),
                move |shell: &mut Shell, swipe: gestures::Swipe| shell.swiped(tab, swipe),
                move |shell: &mut Shell, phase: GesturePhase, scale: f32| {
                    shell.pinched(tab, phase, scale)
                },
            );
            crate::requests::framed(shell, tab, caught.keyed(("page", tab)))
        }
        Page::Unloaded | Page::Discarded(_) => container(spacer())
            .background(palette().background)
            .fill()
            .any(),
    }
}

pub(crate) fn progress(shell: &Shell) -> Element {
    let model = shell.model();
    let Some(tab_entry) = model.active(shell.id()) else {
        return Element::empty();
    };
    if !*tab_entry.meta.loading || tab_entry.internal.is_some() {
        return Element::empty();
    }
    let done = tab_entry.meta.progress.clamp(0.08, 1.0);
    container(spacer())
        .background(palette().accent)
        .height(PROGRESS)
        .width(Length::portion(done))
        .build()
        .animate(Motion::Standard)
        .position(0.0, 0.0)
}

fn empty() -> Element {
    status_page(MaterialSymbol::TAB, tr!("browser.empty.title"))
        .action(
            button(text(tr!("browser.empty.new-tab")))
                .primary()
                .on_press(|shell: &mut Shell| shell.new_tab()),
        )
        .any()
}
