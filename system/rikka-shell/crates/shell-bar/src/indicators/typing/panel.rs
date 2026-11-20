use rikka_ime::{Property, StatusArea};
use rikka_session::settings_app::{Link, Spot};
use rikka_theme::Corner;
use rikka_ui::{
    Align, Inset, Justify, Length, MaterialSymbol, Space, Styled, TextStyled, View, palette, text,
    tr,
};
use rikka_widgets::{column, container, row};
use shell_ime::InputMethods;

use crate::dropdown_panel::{ROW_DEPTH, ROW_INSET};
use crate::indicators::Shelf;
use crate::indicators::parts::{Entry, entry, heading, rule};
use crate::indicators::typing::mark;

const BADGE: f32 = 28.0;

pub(crate) fn view(methods: &InputMethods, shelf: &Shelf) -> impl View {
    let area = methods.status_area();
    let current = area.current();
    column! {
        current.map(header);
        if !area.input_methods.is_empty() {
            heading(tr!("shell.indicators.input-method.methods"));
            for method in &area.input_methods {
                choice(methods, shelf, method, current);
            }
        }
        if area.actions().next().is_some() {
            rule();
            heading(tr!("shell.indicators.input-method.modes"));
            for action in area.actions() {
                action_rows(methods, shelf, &area, action);
            }
        }
        rule();
        entry(tr!("shell.indicators.input-method.settings"), {
            let shelf = shelf.clone();
            move || shelf.launch(Link::Spot(Spot::InputSources).argv())
        })
        .trailing(MaterialSymbol::OPEN_IN_NEW);
    }
    .gap(Space::HAIR)
    .align_items(Align::Stretch)
    .fill_width()
}

fn header(current: &Property) -> impl View {
    let palette = palette();
    row! {
        mark::badge(current).map(|badge| {
            container(
                row! { badge }
                    .justify(Justify::Center)
                    .align_items(Align::Center)
                    .size(Length::px(BADGE), Length::px(BADGE)),
            )
            .background(palette.surface_variant)
            .radius(Corner::Item)
        });
        column! {
            text(current.label.as_str()).label().medium().no_wrap();
            if !current.tip.is_empty() {
                text(current.tip.as_str()).caption().muted().no_wrap();
            }
        }
        .gap(Space::HAIR)
        .grow(1.0);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
    .padding(Inset::xy(ROW_INSET, ROW_DEPTH))
}

fn choice(
    methods: &InputMethods,
    shelf: &Shelf,
    method: &Property,
    current: Option<&Property>,
) -> impl View {
    let (methods, shelf, key) = (methods.clone(), shelf.clone(), method.key.clone());
    entry(method.label.as_str(), move || {
        methods.trigger(&key);
        shelf.close();
    })
    .checked(current.is_some_and(|current| current.label == method.label))
}

fn action_rows(
    methods: &InputMethods,
    shelf: &Shelf,
    area: &StatusArea,
    action: &Property,
) -> impl View {
    let unfolded = area.menu(&action.key);
    let tip = area.current().map(|current| current.tip.as_str());
    column! {
        unfolding(methods, shelf, action, unfolded.is_some());
        for item in unfolded.unwrap_or_default() {
            command(methods, shelf, item, tip);
        }
    }
    .gap(Space::HAIR)
    .align_items(Align::Stretch)
    .fill_width()
}

fn unfolding(
    methods: &InputMethods,
    shelf: &Shelf,
    action: &Property,
    unfolded: bool,
) -> impl View {
    if !action.opens_menu() {
        return command(methods, shelf, action, None).depth(0);
    }
    let (methods, key) = (methods.clone(), action.key.clone());
    let row = described(
        entry(action.label.as_str(), move || match unfolded {
            true => methods.fold(),
            false => methods.trigger(&key),
        }),
        action,
    );
    row.trailing(match unfolded {
        true => MaterialSymbol::EXPAND_LESS,
        false => MaterialSymbol::EXPAND_MORE,
    })
}

fn command(methods: &InputMethods, shelf: &Shelf, item: &Property, tip: Option<&str>) -> Entry {
    let (methods, shelf, key) = (methods.clone(), shelf.clone(), item.key.clone());
    described(
        entry(item.label.as_str(), move || {
            methods.trigger(&key);
            shelf.close();
        }),
        item,
    )
    .checked(tip.is_some_and(|tip| !tip.is_empty() && tip == item.label))
    .depth(1)
}

fn described(row: Entry, property: &Property) -> Entry {
    match property.tip.is_empty() || property.tip == property.label {
        true => row,
        false => row.detail(property.tip.as_str()),
    }
}
