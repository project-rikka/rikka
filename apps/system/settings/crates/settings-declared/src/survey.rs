use rikka_settings::catalog::Entry;
use rikka_settings::{Catalog, Layers, Source, Store, Value};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SettingId {
    pub domain: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Setting {
    pub id: SettingId,
    pub instance: Option<String>,
    pub key: String,
    pub summary: Option<String>,
    pub value: Value,
    pub source: Source,
    pub user: Option<Value>,
    pub beneath: Value,
    pub beneath_source: Source,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    pub domain: String,
    pub key: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Survey {
    settings: Vec<Setting>,
    problems: Vec<Problem>,
}

impl Setting {
    #[must_use]
    pub fn is_changed(&self) -> bool {
        !self.source.is_locked()
            && self
                .user
                .as_ref()
                .is_some_and(|value| *value != self.beneath)
    }

    #[must_use]
    pub fn is_declared(&self) -> bool {
        self.source.is_declared() || self.beneath_source.is_declared()
    }

    #[must_use]
    pub fn is_locked(&self) -> bool {
        self.source.is_locked()
    }
}

impl Survey {
    #[must_use]
    pub fn read(catalog: &Catalog, layers: &Layers) -> Self {
        let mut survey = Self::default();
        for entry in catalog.entries() {
            match entry.open(layers.clone()) {
                Ok((store, report)) => {
                    survey
                        .problems
                        .extend(report.rejected.into_iter().map(|rejection| Problem {
                            domain: entry.domain().to_owned(),
                            key: rejection.key,
                            reason: rejection.reason,
                        }));
                    survey.settings.extend(settings(entry, &store));
                }
                Err(error) => survey.problems.push(Problem {
                    domain: entry.domain().to_owned(),
                    key: None,
                    reason: error.to_string(),
                }),
            }
        }
        survey
    }

    #[must_use]
    pub fn settings(&self) -> &[Setting] {
        &self.settings
    }

    pub fn changed(&self) -> impl Iterator<Item = &Setting> {
        self.settings.iter().filter(|setting| setting.is_changed())
    }

    pub fn declared(&self) -> impl Iterator<Item = &Setting> {
        self.settings.iter().filter(|setting| setting.is_declared())
    }

    #[must_use]
    pub fn problems(&self) -> &[Problem] {
        &self.problems
    }

    #[must_use]
    pub fn find(&self, id: &SettingId) -> Option<&Setting> {
        self.settings.iter().find(|setting| setting.id == *id)
    }
}

fn settings<'a>(entry: &'a Entry, store: &'a Store) -> impl Iterator<Item = Setting> + 'a {
    store
        .schema()
        .keys()
        .iter()
        .filter(|key| key.is_preference())
        .filter_map(move |key| {
            let name = key.name();
            let setting = entry.setting(name)?;
            let resolved = store.resolve(name).ok()?;
            let beneath = store.beneath_user(name).ok()?;
            Some(Setting {
                id: SettingId {
                    domain: entry.domain().to_owned(),
                    name: name.to_owned(),
                },
                instance: setting.instance.map(str::to_owned),
                key: setting.key.name().to_owned(),
                summary: setting.key.summary_text().map(str::to_owned),
                value: resolved.value.clone(),
                source: resolved.source,
                user: store.user_values().get(name).cloned(),
                beneath: beneath.value.clone(),
                beneath_source: beneath.source,
            })
        })
}
