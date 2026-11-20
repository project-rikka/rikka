use rikka_app::prelude::*;
use rikka_ime::{Engine, InputSource};
use rikka_input::Layout;

use crate::keyboard::component::Keyboard;
use crate::keyboard::packages::{self, OFFERED, Package};
use crate::pending::Pending;
use crate::settings::Settings;

const METHODS: &str = "settings.keyboard.methods";
const LAYOUTS: &str = "settings.keyboard.layouts";
const INSTALL: &str = "settings.keyboard.install";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Offer {
    Heading(&'static str),
    Engine(String),
    Layout(String),
    Package(&'static Package),
}

pub(crate) fn offers(keyboard: &Keyboard, pending: &Pending) -> Vec<Offer> {
    let query = keyboard.query().to_lowercase();
    let mut offers = Vec::new();

    let methods = keyboard
        .composing()
        .into_iter()
        .filter(|engine| names(engine).iter().any(|name| matches(name, &query)))
        .map(|engine| Offer::Engine(engine.id.clone()));
    extend(&mut offers, METHODS, methods);

    let current = keyboard.sources();
    let layouts = keyboard
        .layouts()
        .all()
        .iter()
        .filter(|layout| !has_plain_layout(&current, layout))
        .filter(|layout| matches(&layout.description, &query) || matches(&layout.name, &query))
        .map(|layout| Offer::Layout(layout.name.clone()));
    extend(&mut offers, LAYOUTS, layouts);

    if pending.unavailable().is_none() {
        let installed = packages::installed(pending);
        let install = OFFERED
            .iter()
            .filter(|package| {
                keyboard.stages(package.attribute)
                    || !packages::installs(&installed, package.attribute)
            })
            .filter(|package| matches(&Label::translated(package.label), &query))
            .map(Offer::Package);
        extend(&mut offers, INSTALL, install);
    }

    offers
}

pub(crate) fn rows(keyboard: &Keyboard, offers: &[Offer]) -> Vec<ListItem> {
    offers
        .iter()
        .enumerate()
        .map(|(index, offer)| row(keyboard, offer, index))
        .collect()
}

fn row(keyboard: &Keyboard, offer: &Offer, index: usize) -> ListItem {
    match offer {
        Offer::Heading(title) => list_header(Label::translated(title)),
        Offer::Engine(id) => engine_row(keyboard, id),
        Offer::Layout(name) => layout_row(keyboard, name),
        Offer::Package(package) => package_row(keyboard, package, index),
    }
}

fn engine_row(keyboard: &Keyboard, id: &str) -> ListItem {
    let Some(engine) = keyboard.engine(id) else {
        return list_item(id);
    };
    let row = list_item(engine.name.as_str());
    match language_name(engine) {
        Some(language) => row.secondary(language),
        None => row,
    }
}

fn layout_row(keyboard: &Keyboard, name: &str) -> ListItem {
    let described = keyboard
        .layouts()
        .layout(name)
        .map(|layout| layout.description.clone())
        .unwrap_or_else(|| name.to_owned());
    list_item(described).secondary(name)
}

fn package_row(keyboard: &Keyboard, package: &'static Package, index: usize) -> ListItem {
    let staged = keyboard.stages(package.attribute);
    let row = list_item(Label::translated(package.label))
        .check(staged)
        .on_check(move |settings: &mut Settings, _: bool| settings.choose_offer(index));
    match staged {
        true => row.trailing(tr!("settings.keyboard.after-apply")),
        false => row,
    }
}

fn extend(offers: &mut Vec<Offer>, heading: &'static str, group: impl Iterator<Item = Offer>) {
    for (nth, offer) in group.enumerate() {
        if nth == 0 {
            offers.push(Offer::Heading(heading));
        }
        offers.push(offer);
    }
}

fn names(engine: &Engine) -> [&str; 3] {
    [
        engine.name.as_str(),
        engine.native_name.as_str(),
        engine.language.as_str(),
    ]
}

fn language_name(engine: &Engine) -> Option<String> {
    [&engine.native_name, &engine.language]
        .into_iter()
        .find(|name| !name.is_empty())
        .cloned()
}

fn has_plain_layout(sources: &[InputSource], layout: &Layout) -> bool {
    sources.iter().any(|source| {
        source.is_plain() && source.variant.is_empty() && source.layout == layout.name
    })
}

fn matches(name: &str, query: &str) -> bool {
    query.is_empty() || name.to_lowercase().contains(query)
}
