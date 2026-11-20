use rikka_app::prelude::*;
use rikka_nix::Component;
use rikka_widgets::column;

use crate::apps::Apps;
use crate::manager::Manager;
use crate::pane;

pub(crate) fn view(apps: &Apps, busy: bool) -> Element {
    let Some(distribution) = apps.distribution() else {
        return container(status_page(
            MaterialSymbol::APPS,
            tr!("packages.empty.apps"),
        ))
        .center()
        .fill()
        .build();
    };
    scrollable(
        container(
            column! {
                for component in distribution.components() {
                    entry(component, apps, busy);
                }
            }
            .gap(Space::SNUG)
            .fill_width(),
        )
        .padding(Inset::xy(14.0, 12.0))
        .fill_width(),
    )
    .fill()
    .build()
}

fn entry(component: &Component, apps: &Apps, busy: bool) -> impl View {
    let optional = component.is_optional();
    let id = component.id.clone();
    pane::card(
        row! {
            checkbox(!optional || apps.is_included(&component.id))
                .enabled(optional && !busy)
                .on_toggle(move |manager: &mut Manager, on: bool| manager.choose_app(&id, on));
            column! {
                text(component.name.clone()).label().semibold();
                text(component.summary.clone()).caption().muted();
            }
            .gap(Space::HAIR)
            .grow(1.0);
            if !optional {
                pane::badge(tr!("packages.apps.required").to_string(), palette().text_muted);
            }
            text(component.version.clone()).caption().muted().no_wrap();
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
}
