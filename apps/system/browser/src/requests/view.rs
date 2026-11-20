use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::interstitial;
use crate::overlays;
use crate::requests::menus::{choice_view, context_view};
use crate::requests::sheets::sheet;
use crate::requests::{DropdownOwner, dropdown_view};
use crate::shell::Shell;
use browser_model::TabId;

pub(crate) type Mark = (&'static str, TabId);

const PAGE_TOP: &str = "rikka-browser-page-top";
const EDGE: f32 = 8.0;
const HINT_DROP: f32 = 56.0;

pub(crate) fn framed(shell: &Shell, tab: TabId, page: Element) -> Element {
    let mark: Mark = (PAGE_TOP, tab);
    let mut layers = vec![
        page,
        spacer()
            .width(Length::FILL)
            .height(0.0)
            .position(0.0, 0.0)
            .build()
            .keyed(mark),
    ];
    let site = shell
        .model()
        .tab(tab)
        .map(|tab_entry| crate::address::display_address(&tab_entry.meta.url).host)
        .unwrap_or_default();
    let page_services = shell.browser().page_services();
    let requests = page_services.requests();
    if let Some(tab_requests) = requests.get(tab) {
        if let Some(problem) = tab_requests.failure.problem() {
            layers.push(
                interstitial::view(problem, tab)
                    .position(0.0, 0.0)
                    .fill()
                    .build(),
            );
        } else if tab_requests.failure.hung() {
            layers.push(
                hung(tab)
                    .anchored(Anchor::new(mark, AnchorEdge::Center).offset(0.0, HINT_DROP * 2.0)),
            );
        }
        if tab_requests.find.is_open() {
            layers.push(
                crate::find::bar(
                    &tab_requests.find,
                    tab,
                    shell.browser().preferences().find_options(),
                )
                .anchored(Anchor::new(mark, AnchorEdge::BelowEnd).offset(-EDGE, EDGE)),
            );
        }
        layers.extend(sheet(tab_requests, tab, &site));
        layers.extend(context_view(shell, tab_requests, tab, mark));
        layers.extend(choice_view(tab_requests, tab, mark));
        layers.extend(dropdown_view(
            &tab_requests.dropdown,
            DropdownOwner::Tab(tab),
        ));
    }
    drop(requests);
    layers.extend(crate::status::hovered(shell, tab));
    layers.extend(crate::status::tooltip(shell, tab, mark));
    if let Some(hint) = overlays::hint(shell) {
        layers.push(hint.anchored(Anchor::new(mark, AnchorEdge::Center).offset(0.0, HINT_DROP)));
    }
    column_of(layers).fill().any()
}

fn hung(tab: TabId) -> Element {
    container(
        column! {
            text(tr!("browser.hung.title")).label().semibold();
            row! {
                button(text(tr!("browser.hung.end")))
                    .secondary()
                    .small()
                    .on_press(move |shell: &mut Shell| shell.end_page(tab));
                button(text(tr!("browser.hung.wait")))
                    .primary()
                    .small()
                    .on_press(move |shell: &mut Shell| shell.wait_for_page(tab));
            }
            .gap(Space::SNUG);
        }
        .gap(Space::NORMAL),
    )
    .padding(Inset::all(Space::NORMAL))
    .background(palette().surface)
    .border(1.0, palette().outline.with_alpha(0.6))
    .radius(Corner::Panel)
    .any()
}
