use nixpkgs_search::Package;
use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::listing::Line;
use crate::manager::Manager;
use crate::pane::{self, card, details};

pub(crate) fn view(line: &Line<'_>, package: Option<&Package>, busy: bool) -> impl View {
    let body = match package {
        None => column! {
            header(line);
            actions(line, busy);
            status_page(MaterialSymbol::SEARCH_OFF, tr!("packages.detail.unknown"))
                .tint(palette().warning);
        },
        Some(found) => column! {
            header(line);
            actions(line, busy);
            if !found.description.is_empty() {
                text(found.description.clone())
                    .label()
                    .color(palette().text)
                    .line_height(20.0)
                    .fill_width();
            }
            metadata(found);
            if !found.about.is_empty() {
                about(found);
            }
            maintainers(found);
            homepages(found);
        },
    };

    scrollable(
        container(body.gap(Space::ROOMY).fill_width())
            .padding(Inset::xy(18.0, 18.0))
            .fill_width(),
    )
    .fill()
}

fn header(line: &Line<'_>) -> impl View {
    column! {
        text(line.name.to_owned()).title().semibold();
        row! {
            if !line.version.is_empty() {
                text(line.version.to_owned()).label().muted();
            }
            if let Some(label) = line.mark.label() {
                pane::badge(label.to_string(), line.mark.tint(&palette()));
            }
            if line.unfree {
                pane::badge(tr!("packages.detail.unfree").to_string(), palette().warning);
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center);
    }
    .gap(Space::TIGHT)
    .fill_width()
}

fn actions(line: &Line<'_>, busy: bool) -> impl View {
    row! {
        choice(line, busy);
        if line.mark.is_pending() {
            revert(line.attribute.to_owned(), busy);
        }
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn choice(line: &Line<'_>, busy: bool) -> Button {
    let attribute = line.attribute.to_owned();
    match line.mark.is_included() {
        true => button(text(tr!("packages.action.remove")))
            .danger()
            .enabled(!busy)
            .on_press(move |manager: &mut Manager| manager.exclude(&attribute)),
        false => button(text(tr!("packages.action.install")))
            .primary()
            .enabled(!busy)
            .on_press(move |manager: &mut Manager| manager.include(&attribute)),
    }
}

fn revert(attribute: String, busy: bool) -> Button {
    button(text(tr!("packages.action.revert")))
        .ghost()
        .enabled(!busy)
        .on_press(move |manager: &mut Manager| manager.revert(&attribute))
}

fn metadata(found: &Package) -> impl View {
    let mut rows: Vec<(Label, String)> =
        vec![(tr!("packages.detail.attribute"), found.attribute.clone())];
    if let Some(set) = &found.set {
        rows.push((tr!("packages.detail.set"), set.clone()));
    }
    if !found.licenses.is_empty() {
        rows.push((
            tr!("packages.detail.licences"),
            joined(found.licenses.iter().map(|licence| licence.label())),
        ));
    }
    if let Some(main) = &found.main_program {
        rows.push((tr!("packages.detail.main-program"), main.clone()));
    }
    if !found.outputs.is_empty() {
        rows.push((
            tr!("packages.detail.outputs"),
            joined(found.outputs.iter().map(String::as_str)),
        ));
    }
    if !found.platforms.is_empty() {
        rows.push((
            tr!("packages.detail.platforms"),
            found.platforms.len().to_string(),
        ));
    }
    if let Some(position) = &found.position {
        rows.push((tr!("packages.detail.definition"), position.clone()));
    }

    card(details(rows))
}

fn about(found: &Package) -> impl View {
    card(
        column! {
            text(tr!("packages.detail.about")).label().semibold();
            text(found.about.clone())
                .caption()
                .muted()
                .line_height(19.0)
                .fill_width();
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
}

fn maintainers(found: &Package) -> Option<impl View> {
    let people: Vec<&str> = found
        .maintainers
        .iter()
        .filter_map(nixpkgs_search::Maintainer::label)
        .collect();
    if people.is_empty() && found.teams.is_empty() {
        return None;
    }

    let mut rows: Vec<(Label, String)> = Vec::new();
    if !people.is_empty() {
        rows.push((
            tr!("packages.detail.maintainers"),
            joined(people.into_iter()),
        ));
    }
    for team in &found.teams {
        rows.push((
            tr!("packages.detail.team"),
            match team.scope.is_empty() {
                true => team.name.clone(),
                false => format!("{} — {}", team.name, team.scope),
            },
        ));
    }
    Some(card(details(rows)))
}

fn homepages(found: &Package) -> Option<impl View> {
    if found.homepages.is_empty() {
        return None;
    }
    Some(
        column! {
            for page in &found.homepages {
                link(page.clone());
            }
        }
        .gap(Space::TIGHT)
        .fill_width(),
    )
}

fn link(page: String) -> impl View {
    button(
        row! {
            icon(MaterialSymbol::LINK).extent(Length::px(15.0));
            text(page.clone()).caption().no_wrap();
        }
        .gap(Space::SNUG)
        .align_items(Align::Center),
    )
    .ghost()
    .small()
    .on_press(move || browse(&page))
}

fn browse(page: &str) {
    if let Err(error) = rikka_open::open(page) {
        tracing::warn!(%error, page, "failed to open homepage");
    }
}

fn joined<'a>(entries: impl Iterator<Item = &'a str>) -> String {
    entries.collect::<Vec<&str>>().join("  ")
}
