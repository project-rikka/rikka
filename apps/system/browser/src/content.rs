use rikka_app::prelude::*;
use rikka_web::{NavigationButton, web_view};

use crate::chrome::Phase;
use crate::model::Tab;
use crate::page::Page;
use crate::shell::Shell;
use crate::{gestures, internal, keys, peek};
use browser_model::TabId;

const PROGRESS: f32 = 2.0;
const PAGE_TOP: &str = "rikka-browser-page-top";
const PAGE_EDGE: &str = "rikka-browser-page-edge";
const HINT_DROP: f32 = 56.0;
const KEYBOARD_MENU: Point = Point::new(Space::LOOSE.px(), Space::LOOSE.px());

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
        crate::status::hovered(shell, tab),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn window_overlays(shell: &Shell) -> Vec<Element> {
    peek::view(shell).into_iter().collect()
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
    match shell.browser().services().chrome.phase(tab_entry.profile) {
        Some(Phase::Failed(failure)) => return crate::chrome::failure_page(&failure),
        Some(Phase::Exited(_)) if tab_entry.page().live().is_none() => {
            return crate::chrome::exited_page();
        }
        _ => {}
    }
    if active == Some(tab_entry.id)
        && let Some(failure) = shell.engine_error()
    {
        return crate::chrome::unavailable_page(failure);
    }
    if let Some(reading) = crate::reader::view(shell, tab_entry.id) {
        return reading;
    }
    let loading = *tab_entry.meta.loading;
    let tab = tab_entry.id;
    match tab_entry.page() {
        Page::Live(live) => {
            let surface = web_view(&live.view)
                .reserve_shortcuts(keys::reserved(loading).chain(peek::reserved(shell, tab)))
                .on_navigation_button(|shell: &mut Shell, button: NavigationButton| {
                    shell.navigation_button(button)
                })
                .fill();
            let page = gestures::catch_gestures(
                crate::split::focusing(surface, tab),
                move |shell: &mut Shell, swipe: gestures::Swipe| shell.swiped(tab, swipe),
                move |shell: &mut Shell, phase: GesturePhase, scale: f32| {
                    shell.pinched(tab, phase, scale)
                },
            )
            .keyed(("page", tab));
            hinted(shell, tab, page)
        }
        Page::Unloaded => container(spacer())
            .background(palette().background)
            .fill()
            .any(),
    }
}

fn hinted(shell: &Shell, tab: TabId, page: Element) -> Element {
    let mark = (PAGE_TOP, tab);
    let hint = crate::overlays::hint(shell)
        .map(|hint| hint.anchored(Anchor::new(mark, AnchorEdge::Center).offset(0.0, HINT_DROP)));
    let layers = [
        page,
        spacer()
            .width(Length::FILL)
            .height(0.0)
            .position(0.0, 0.0)
            .build()
            .keyed(mark),
        spacer()
            .width(0.0)
            .height(Length::FILL)
            .position(0.0, 0.0)
            .build()
            .keyed((PAGE_EDGE, tab)),
    ];
    column_of(layers.into_iter().chain(hint)).fill().any()
}

pub(crate) fn menu_anchor(tab: TabId, at: Option<Point>) -> Anchor {
    let at = at.unwrap_or(KEYBOARD_MENU);
    Anchor::new((PAGE_EDGE, tab), AnchorEdge::BesideTop).offset(at.x, at.y)
}

pub(crate) fn progress(shell: &Shell) -> Element {
    let model = shell.model();
    let Some(tab_entry) = model.active(shell.id()) else {
        return Element::empty();
    };
    if !*tab_entry.meta.loading || tab_entry.internal.is_some() {
        return Element::empty();
    }
    progress_indeterminate()
        .fill_width()
        .height(PROGRESS)
        .build()
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
