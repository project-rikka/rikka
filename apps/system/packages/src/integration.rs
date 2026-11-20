use std::collections::{BTreeMap, BTreeSet};

use nixpkgs_search::{Requirement, Setting, Suggestion};
use rikka_nix::{AttrPath, Document, Expr};

use crate::mark::Mark;

#[derive(Clone, Debug)]
struct Offer {
    suggestions: Vec<Suggestion>,
    accepted: BTreeSet<String>,
    installer: Option<String>,
}

impl Offer {
    fn has_requirement(&self, key: &str) -> bool {
        self.suggestions
            .iter()
            .any(|suggestion| suggestion.requirement.key() == key)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Row {
    pub(crate) package: String,
    pub(crate) requirement: Requirement,
    pub(crate) on: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Groups {
    path: AttrPath,
    names: BTreeSet<String>,
}

#[derive(Debug)]
pub(crate) struct Writes {
    pub(crate) switches: BTreeMap<String, Setting>,
    pub(crate) groups: Option<Groups>,
    pub(crate) removed: BTreeSet<String>,
    pub(crate) unfree: bool,
}

impl Writes {
    pub(crate) fn apply_to(&self, document: &mut Document) -> rikka_nix::Result<()> {
        for (path, setting) in &self.switches {
            document.set(path, value(setting))?;
        }
        if let Some(groups) = &self.groups
            && !groups.names.is_empty()
        {
            document.set(&groups.path, Expr::list(&groups.names))?;
        }
        for path in &self.removed {
            document.remove(path)?;
        }
        Ok(())
    }
}

fn value(setting: &Setting) -> Expr {
    match setting {
        Setting::Enabled => Expr::Bool(true),
        Setting::Value(value) => Expr::from(value),
    }
}

#[derive(Debug, Default)]
struct Current {
    declared_paths: BTreeSet<String>,
    switches: BTreeMap<String, bool>,
    strings: BTreeMap<String, String>,
    groups: BTreeSet<String>,
}

impl Current {
    fn read(document: &Document, groups: Option<&AttrPath>) -> rikka_nix::Result<Self> {
        let groups = match groups {
            Some(path) => document.get::<Vec<String>>(path)?.unwrap_or_default(),
            None => Vec::new(),
        };
        let mut current = Self {
            groups: groups.into_iter().collect(),
            ..Self::default()
        };
        for path in document.paths()? {
            let key = path.to_string();
            match document.get::<Expr>(&path)? {
                Some(Expr::Bool(on)) => {
                    current.switches.insert(key.clone(), on);
                }
                Some(Expr::String(value)) => {
                    current.strings.insert(key.clone(), value);
                }
                _ => {}
            }
            current.declared_paths.insert(key);
        }
        Ok(current)
    }
}

#[derive(Debug)]
pub(crate) struct Integration {
    login: Option<String>,
    current: Current,
    unfree: bool,
    offered: BTreeMap<String, Offer>,
}

impl Integration {
    pub(crate) fn new(login: Option<String>) -> Self {
        Self {
            login,
            current: Current::default(),
            unfree: false,
            offered: BTreeMap::new(),
        }
    }

    pub(crate) fn declared(&mut self, document: &Document, unfree: bool) -> rikka_nix::Result<()> {
        self.current = Current::read(document, self.groups_path().as_ref())?;
        self.unfree = unfree;
        self.reset_accepted();
        Ok(())
    }

    pub(crate) fn enabled(&self) -> impl Iterator<Item = &str> {
        self.current
            .switches
            .iter()
            .filter(|(_, on)| **on)
            .map(|(path, _)| path.as_str())
    }

    pub(crate) fn offer(
        &mut self,
        package: &str,
        suggestions: Vec<Suggestion>,
        installer: Option<String>,
        mark: Mark,
    ) {
        let fresh = mark == Mark::Adding;
        let suggestions: Vec<Suggestion> = suggestions
            .into_iter()
            .filter(|suggestion| {
                self.login.is_some() || !matches!(suggestion.requirement, Requirement::Group(_))
            })
            .collect();
        if suggestions.is_empty() {
            return;
        }
        let previous = self.offered.get(package);
        let accepted = suggestions
            .iter()
            .filter(|suggestion| {
                let key = suggestion.requirement.key();
                match previous.filter(|previous| previous.has_requirement(&key)) {
                    Some(previous) => previous.accepted.contains(&key),
                    None => {
                        self.is_satisfied(&suggestion.requirement)
                            || (fresh && suggestion.recommended)
                    }
                }
            })
            .map(|suggestion| suggestion.requirement.key())
            .collect();
        self.offered.insert(
            package.to_owned(),
            Offer {
                suggestions,
                accepted,
                installer,
            },
        );
    }

    pub(crate) fn choose(&mut self, package: &str, key: &str, on: bool) {
        let Some(offer) = self.offered.get_mut(package) else {
            return;
        };
        if on {
            offer.accepted.insert(key.to_owned());
        } else {
            offer.accepted.remove(key);
        }
    }

    pub(crate) fn discard(&mut self) {
        self.reset_accepted();
    }

    pub(crate) fn installed(&self, package: &str) -> bool {
        let Some(offer) = self.offered.get(package) else {
            return false;
        };
        offer
            .installer
            .as_ref()
            .is_some_and(|path| offer.accepted.contains(path))
    }

    pub(crate) fn rows(&self, selected: &BTreeSet<String>, fresh: &BTreeSet<String>) -> Vec<Row> {
        let mut rows = Vec::new();
        for (package, offer) in &self.offered {
            if !selected.contains(package) {
                continue;
            }
            for suggestion in &offer.suggestions {
                let on = offer.accepted.contains(&suggestion.requirement.key());
                if !fresh.contains(package) && on == self.is_satisfied(&suggestion.requirement) {
                    continue;
                }
                rows.push(Row {
                    package: package.clone(),
                    requirement: suggestion.requirement.clone(),
                    on,
                });
            }
        }
        rows
    }

    pub(crate) fn writes(&self, selected: &BTreeSet<String>) -> Writes {
        let mut switches = BTreeMap::new();
        let mut names = BTreeSet::new();
        let mut unfree = self.unfree;

        for (package, offer) in &self.offered {
            if !selected.contains(package) {
                continue;
            }
            for suggestion in &offer.suggestions {
                if !offer.accepted.contains(&suggestion.requirement.key()) {
                    continue;
                }
                match &suggestion.requirement {
                    Requirement::Option { path, setting, .. } => {
                        switches.insert(path.clone(), setting.clone());
                    }
                    Requirement::Group(name) => {
                        names.insert(name.clone());
                    }
                    Requirement::Unfree => unfree = true,
                }
            }
        }

        let groups = self.groups_path().map(|path| Groups { path, names });
        let removed = self.removed(&switches, groups.as_ref());
        Writes {
            switches,
            groups,
            removed,
            unfree,
        }
    }

    pub(crate) fn is_unchanged(&self, selected: &BTreeSet<String>) -> bool {
        let writes = self.writes(selected);
        if !writes.removed.is_empty() || (writes.unfree && !self.unfree) {
            return false;
        }
        let all_set = writes
            .switches
            .iter()
            .all(|(path, setting)| self.is_set(path, setting));
        match &writes.groups {
            Some(groups) if !groups.names.is_empty() => {
                all_set && self.current.groups == groups.names
            }
            _ => all_set,
        }
    }

    fn removed(
        &self,
        switches: &BTreeMap<String, Setting>,
        groups: Option<&Groups>,
    ) -> BTreeSet<String> {
        let mut removed = BTreeSet::new();
        let mut offers_group = false;
        for offer in self.offered.values() {
            for suggestion in &offer.suggestions {
                match &suggestion.requirement {
                    Requirement::Option { path, .. } => {
                        if !switches.contains_key(path)
                            && self.current.declared_paths.contains(path)
                        {
                            removed.insert(path.clone());
                        }
                    }
                    Requirement::Group(_) => offers_group = true,
                    Requirement::Unfree => {}
                }
            }
        }
        if let Some(groups) = groups
            && offers_group
            && groups.names.is_empty()
            && self
                .current
                .declared_paths
                .contains(&groups.path.to_string())
        {
            removed.insert(groups.path.to_string());
        }
        removed
    }

    fn reset_accepted(&mut self) {
        let accepted: Vec<(String, BTreeSet<String>)> = self
            .offered
            .iter()
            .map(|(package, offer)| {
                let accepted = offer
                    .suggestions
                    .iter()
                    .filter(|suggestion| self.is_satisfied(&suggestion.requirement))
                    .map(|suggestion| suggestion.requirement.key())
                    .collect();
                (package.clone(), accepted)
            })
            .collect();
        for (package, accepted) in accepted {
            if let Some(offer) = self.offered.get_mut(&package) {
                offer.accepted = accepted;
            }
        }
    }

    fn is_satisfied(&self, requirement: &Requirement) -> bool {
        match requirement {
            Requirement::Option { path, setting, .. } => self.is_set(path, setting),
            Requirement::Group(name) => self.current.groups.contains(name),
            Requirement::Unfree => self.unfree,
        }
    }

    fn is_set(&self, path: &str, setting: &Setting) -> bool {
        match setting {
            Setting::Enabled => self.current.switches.get(path) == Some(&true),
            Setting::Value(value) => self.current.strings.get(path) == Some(value),
        }
    }

    fn groups_path(&self) -> Option<AttrPath> {
        let login = self.login.as_deref()?;
        AttrPath::new(["users", "users", login, "extraGroups"]).ok()
    }
}
