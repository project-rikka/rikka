use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::defaults::catalogue::{CATEGORIES, Catalogue, Choice, Kind, icon_of};
use crate::defaults::component::Defaults;
use crate::defaults::pane::Pane;
use crate::notice::notice;
use crate::picker::picker;

const KINDS: f32 = 360.0;

pub(crate) fn view(defaults: &Defaults) -> impl View {
    column! {
        defaults.failure().map(|reason| {
            notice(
                MaterialSymbol::ERROR,
                palette().danger,
                tr!("settings.defaults.refused", reason = reason),
            )
        });
        segmented(defaults.pane().index())
            .segments(Pane::ALL.map(Pane::title))
            .on_select(|defaults: &mut Defaults, index: usize| {
                defaults.show(Pane::from_index(index));
            })
            .fill_width();
        match defaults.catalogue() {
            None => row! {
                spinner();
                text(tr!("settings.defaults.reading")).label().muted();
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .build(),
            Some(Err(reason)) => notice(
                MaterialSymbol::CLOUD_OFF,
                palette().warning,
                tr!("settings.defaults.unavailable", reason = reason.as_str()),
            )
            .build(),
            Some(Ok(catalogue)) => page_stack(
                defaults.pane().index(),
                match defaults.pane() {
                    Pane::Apps => apps(catalogue).build(),
                    Pane::Files | Pane::Links => kinds(defaults).build(),
                },
            )
            .fill_width()
            .build(),
        }
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn apps(catalogue: &Catalogue) -> impl View {
    column! {
        section(
            tr!("settings.defaults.apps"),
            column! {
                for (index, group) in catalogue.groups.iter().enumerate() {
                    category(index, &group.choice);
                }
                terminal(&catalogue.terminal);
            }
            .gap(Space::SNUG)
            .fill_width(),
        );
        row! {
            spacer().grow(1.0);
            button(tr!("settings.defaults.forget-all"))
                .secondary()
                .small()
                .enabled(catalogue.chooses_any())
                .on_press(Defaults::forget_all);
        }
        .fill_width();
    }
    .gap(Space::NORMAL)
    .fill_width()
}

fn category(index: usize, choice: &Choice) -> impl View {
    let about = &CATEGORIES[index];
    labelled(
        about.symbol,
        Label::translated(&format!("settings.defaults.category.{}", about.slug)),
        chooser(
            choice,
            move |defaults: &mut Defaults, app: usize| defaults.choose_category(index, app),
            move |defaults: &mut Defaults| defaults.forget_category(index),
        ),
    )
}

fn terminal(choice: &Choice) -> impl View {
    labelled(
        MaterialSymbol::TERMINAL,
        tr!("settings.defaults.category.terminal"),
        chooser(choice, Defaults::choose_terminal, Defaults::forget_terminal),
    )
}

fn labelled(symbol: MaterialSymbol, title: Label, control: impl View) -> impl View {
    row! {
        icon(symbol).extent(Length::px(20.0)).color(palette().text_muted);
        text(title).label().no_wrap();
        spacer().grow(1.0);
        control;
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}

fn chooser<A, F>(
    choice: &Choice,
    choose: impl IntoHandler<(usize,), (), A>,
    forget: impl IntoHandler<(), (), F>,
) -> impl View {
    row! {
        if choice.offered.is_empty() {
            text(tr!("settings.defaults.none")).label().muted();
        } else {
            picker(
                choice.position(),
                choice.offered.iter().map(|app| Label::from(app.name.as_str())),
                choose,
            );
        }
        button(icon(MaterialSymbol::RESTART_ALT))
            .ghost()
            .small()
            .icon_only()
            .enabled(choice.chosen)
            .on_press(forget)
            .tooltip(tr!("settings.defaults.forget"));
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
}

fn kinds(defaults: &Defaults) -> impl View {
    let shown = defaults.shown();
    let selected = defaults
        .picked()
        .and_then(|picked| shown.iter().position(|kind| kind.mime == picked.mime));
    column! {
        defaults.picked().map(picked);
        text_field(defaults.search())
            .placeholder(tr!("settings.defaults.search"))
            .on_change(|| {})
            .fill_width();
        if shown.is_empty() {
            text(tr!("settings.defaults.nothing")).label().muted();
        } else {
            list(shown.iter().map(|kind| kind_row(kind)))
                .selected(selected)
                .control_size(ControlSize::Small)
                .on_select(Defaults::pick)
                .height(KINDS)
                .fill_width();
        }
    }
    .gap(Space::NORMAL)
    .fill_width()
}

fn kind_row(kind: &Kind) -> ListItem {
    let row = list_item(kind.title()).keyed(kind.mime.as_str());
    let row = match kind.mime.as_str() != kind.title() {
        true => row.secondary(kind.mime.as_str()),
        false => row,
    };
    match kind.choice.current() {
        Some(app) => row.icon(icon_of(app)).trailing(app.name.as_str()),
        None => row
            .icon(MaterialSymbol::APPS)
            .trailing(tr!("settings.defaults.none")),
    }
}

fn picked(kind: &Kind) -> impl View {
    card(
        column! {
            row! {
                column! {
                    text(kind.title()).body().semibold();
                    text(kind.mime.as_str()).caption().muted();
                }
                .gap(Space::TIGHT)
                .grow(1.0);
                button(icon(MaterialSymbol::CLOSE))
                    .ghost()
                    .small()
                    .icon_only()
                    .on_press(Defaults::unpick);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
            field(
                tr!("settings.defaults.opens-with"),
                chooser(&kind.choice, Defaults::choose_picked, Defaults::forget_picked),
            );
        }
        .gap(Space::NORMAL)
        .fill_width(),
    )
}
