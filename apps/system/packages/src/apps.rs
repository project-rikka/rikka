use std::collections::BTreeSet;

use rikka_nix::{Distribution, Document};

#[derive(Clone, Debug, Default)]
pub(crate) struct Apps {
    distribution: Option<Distribution>,
    installed: BTreeSet<String>,
    included: BTreeSet<String>,
}

impl Apps {
    pub(crate) fn declared(
        &mut self,
        distribution: Option<Distribution>,
        installed: BTreeSet<String>,
    ) {
        let added: Vec<String> = self.included.difference(&self.installed).cloned().collect();
        let removed: Vec<String> = self.installed.difference(&self.included).cloned().collect();
        self.included = installed.clone();
        self.included.extend(added);
        for id in &removed {
            self.included.remove(id);
        }
        self.installed = installed;
        self.distribution = distribution;
        if let Some(distribution) = &self.distribution {
            self.included.retain(|id| {
                distribution
                    .find(id)
                    .is_some_and(|component| component.is_optional())
            });
        }
    }

    pub(crate) fn distribution(&self) -> Option<&Distribution> {
        self.distribution.as_ref()
    }

    pub(crate) fn is_included(&self, id: &str) -> bool {
        self.included.contains(id)
    }

    pub(crate) fn choose(&mut self, id: &str, on: bool) {
        let optional = self
            .distribution
            .as_ref()
            .and_then(|distribution| distribution.find(id))
            .is_some_and(|component| component.is_optional());
        if !optional {
            return;
        }
        match on {
            true => self.included.insert(id.to_owned()),
            false => self.included.remove(id),
        };
    }

    pub(crate) fn discard(&mut self) {
        self.included.clone_from(&self.installed);
    }

    pub(crate) fn is_unchanged(&self) -> bool {
        self.installed == self.included
    }

    pub(crate) fn added(&self) -> Vec<&str> {
        self.named(self.included.difference(&self.installed))
    }

    pub(crate) fn removed(&self) -> Vec<&str> {
        self.named(self.installed.difference(&self.included))
    }

    pub(crate) fn write(&self, apps: &mut Document) -> rikka_nix::Result<()> {
        match &self.distribution {
            Some(distribution) if !self.is_unchanged() => distribution.choose(apps, &self.included),
            _ => Ok(()),
        }
    }

    fn named<'a>(&'a self, ids: impl Iterator<Item = &'a String>) -> Vec<&'a str> {
        ids.map(|id| {
            self.distribution
                .as_ref()
                .and_then(|distribution| distribution.find(id))
                .map_or(id.as_str(), |component| component.name.as_str())
        })
        .collect()
    }
}
