use std::collections::BTreeSet;

use nixpkgs_search::Requirement;
use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::apps::Apps;
use crate::configuration::Changes;
use crate::integration::{Integration, Row};
use crate::job::Job;
use crate::manager::Manager;
use crate::mark::Mark;
use crate::pane;
use crate::work::Work;

const MAX_LISTED_NAMES: usize = 8;

const SUMMARY_LENGTH: usize = 150;

#[derive(Debug)]
pub(crate) struct Plan {
    installed: BTreeSet<String>,
    included: BTreeSet<String>,
    integration: Integration,
    apps: Apps,
}

impl Plan {
    pub(crate) fn new(login: Option<String>) -> Self {
        Self {
            installed: BTreeSet::new(),
            included: BTreeSet::new(),
            integration: Integration::new(login),
            apps: Apps::default(),
        }
    }

    pub(crate) fn declared(&mut self, installed: impl IntoIterator<Item = String>) {
        let added: Vec<String> = self.added().into_iter().map(str::to_owned).collect();
        let removed: Vec<String> = self.removed().into_iter().map(str::to_owned).collect();

        self.installed = installed.into_iter().collect();
        self.included.clone_from(&self.installed);
        self.included.extend(added);
        for package in removed {
            self.included.remove(&package);
        }
    }

    pub(crate) fn integration(&self) -> &Integration {
        &self.integration
    }

    pub(crate) fn integration_mut(&mut self) -> &mut Integration {
        &mut self.integration
    }

    pub(crate) fn apps(&self) -> &Apps {
        &self.apps
    }

    pub(crate) fn apps_mut(&mut self) -> &mut Apps {
        &mut self.apps
    }

    pub(crate) fn include(&mut self, attribute: &str) {
        self.included.insert(attribute.to_owned());
    }

    pub(crate) fn exclude(&mut self, attribute: &str) {
        self.included.remove(attribute);
    }

    pub(crate) fn revert(&mut self, attribute: &str) {
        match self.installed.contains(attribute) {
            true => self.include(attribute),
            false => self.exclude(attribute),
        }
    }

    pub(crate) fn discard(&mut self) {
        self.included.clone_from(&self.installed);
        self.integration.discard();
        self.apps.discard();
    }

    pub(crate) fn mark(&self, attribute: &str) -> Mark {
        match (
            self.installed.contains(attribute),
            self.included.contains(attribute),
        ) {
            (true, true) => Mark::Installed,
            (false, true) => Mark::Adding,
            (true, false) => Mark::Removing,
            (false, false) => Mark::Available,
        }
    }

    pub(crate) fn is_unchanged(&self) -> bool {
        self.installed == self.included
            && self.integration.is_unchanged(&self.included)
            && self.apps.is_unchanged()
    }

    pub(crate) fn changes(&self) -> Changes {
        Changes::new(
            self.system_packages()
                .into_iter()
                .map(str::to_owned)
                .collect(),
            self.integration.writes(&self.included),
            self.apps.clone(),
        )
    }

    fn system_packages(&self) -> Vec<&str> {
        self.included
            .iter()
            .filter(|package| !self.integration.installed(package))
            .map(String::as_str)
            .collect()
    }

    pub(crate) fn listed(&self) -> Vec<&str> {
        self.installed
            .union(&self.included)
            .map(String::as_str)
            .collect()
    }

    pub(crate) fn added(&self) -> Vec<&str> {
        self.included
            .difference(&self.installed)
            .map(String::as_str)
            .collect()
    }

    pub(crate) fn removed(&self) -> Vec<&str> {
        self.installed
            .difference(&self.included)
            .map(String::as_str)
            .collect()
    }

    pub(crate) fn panel(&self, work: &Work) -> Option<Element> {
        let busy = work.is_busy();
        if self.is_unchanged() {
            return None;
        }
        let fresh: BTreeSet<String> = self.added().iter().map(|name| (*name).to_owned()).collect();
        let added: Vec<&str> = self.added().into_iter().chain(self.apps.added()).collect();
        let removed: Vec<&str> = self
            .removed()
            .into_iter()
            .chain(self.apps.removed())
            .collect();
        let rows = self.integration.rows(&self.included, &fresh);

        let body = column! {
            row! {
                icon(MaterialSymbol::CHANGE_CIRCLE)
                    .extent(Length::px(18.0))
                    .color(palette().accent);
                text(tr!("packages.plan.title")).label().semibold();
                spacer().grow(1.0);
                button(text(tr!("packages.action.discard")))
                    .ghost()
                    .enabled(!busy)
                    .on_press(|manager: &mut Manager| manager.discard());
                match work.running(Job::Apply) {
                    Some(stage) => button(pane::working(stage)).primary().enabled(false),
                    None => button(text(tr!("packages.action.apply")))
                        .primary()
                        .enabled(!busy)
                        .on_press(|manager: &mut Manager| manager.apply()),
                };
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
            if !added.is_empty() {
                changed(tr!("packages.plan.adding"), palette().accent, &added);
            }
            if !removed.is_empty() {
                changed(tr!("packages.plan.removing"), palette().danger, &removed);
            }
            if !rows.is_empty() {
                text(tr!("packages.plan.settings"))
                    .caption()
                    .muted()
                    .fill_width();
                for row in &rows {
                    setting(row, busy);
                }
            }
        }
        .gap(Space::SNUG)
        .fill_width();

        Some(pane::panel(body).build())
    }
}

fn setting(row: &Row, busy: bool) -> impl View {
    let (title, detail) = match &row.requirement {
        Requirement::Option {
            path, description, ..
        } => (
            Label::from(path.clone()),
            Some(Label::from(summary(description))),
        ),
        Requirement::Group(name) => (tr!("packages.needs.group", name = name), None),
        Requirement::Unfree => (tr!("packages.needs.unfree"), None),
    };

    let package = row.package.clone();
    let key = row.requirement.key();
    row! {
        checkbox(row.on)
            .control_size(ControlSize::Small)
            .enabled(!busy)
            .on_toggle(move |plan: &mut Plan, on: bool| {
                plan.integration_mut().choose(&package, &key, on);
            });
        column! {
            text(title)
                .caption()
                .family(Family::parse(font::JETBRAINS_MONO))
                .color(palette().text);
            if let Some(detail) = detail {
                text(detail).caption().muted();
            }
        }
        .gap(Space::HAIR)
        .grow(1.0);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}

fn summary(description: &str) -> String {
    let detail: String = description
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ");
    let sentence = match detail.find(". ") {
        Some(end) => &detail[..=end],
        None => detail.as_str(),
    };
    match sentence.char_indices().nth(SUMMARY_LENGTH) {
        Some((end, _)) => format!("{}…", sentence[..end].trim_end()),
        None => sentence.to_owned(),
    }
}

fn changed(title: Label, tint: Color, names: &[&str]) -> impl View {
    let listed = names
        .iter()
        .take(MAX_LISTED_NAMES)
        .copied()
        .collect::<Vec<&str>>()
        .join("  ");
    let rest = names.len().saturating_sub(MAX_LISTED_NAMES);
    let detail = match rest {
        0 => listed,
        rest => tr!(
            "packages.plan.and-more",
            listed = &listed,
            count = &rest.to_string()
        )
        .to_string(),
    };
    row! {
        text(title).caption().color(tint).width(96.0).no_wrap();
        text(detail).caption().color(palette().text).grow(1.0);
    }
    .gap(Space::SNUG)
    .fill_width()
}
