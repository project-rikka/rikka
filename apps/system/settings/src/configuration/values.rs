use std::collections::BTreeMap;

use rikka_seam::SeamClient;
use rikka_settings::seam::protocol;
use rikka_settings::{Layers, Scope, Source, Value};
use settings_declared::{SettingId, Survey};

use crate::configuration::catalog;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Held {
    pub(crate) id: SettingId,
    pub(crate) value: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Write {
    pub(crate) id: SettingId,
    pub(crate) value: Option<Value>,
}

pub(crate) fn survey() -> Result<Survey, String> {
    let layers = Layers::resolve().map_err(|error| error.to_string())?;
    Ok(Survey::read(catalog()?, &layers))
}

#[derive(Debug)]
pub(crate) struct Values<'a> {
    seam: Option<&'a SeamClient>,
    hosted: Vec<String>,
}

impl<'a> Values<'a> {
    pub(crate) fn connect(seam: Option<&'a SeamClient>) -> Result<Values<'a>, String> {
        let hosted = match seam {
            Some(seam) => seam
                .call(protocol::domains())
                .blocking_wait()
                .and_then(|answer| answer.decode())
                .map_err(|error| error.to_string())?,
            None => Vec::new(),
        };
        Ok(Values { seam, hosted })
    }

    pub(crate) fn write(&self, write: Write) -> Result<(), String> {
        match self.write_all(vec![write]).pop() {
            Some(failure) => Err(failure),
            None => Ok(()),
        }
    }

    pub(crate) fn write_all(&self, writes: Vec<Write>) -> Vec<String> {
        let mut domains: BTreeMap<String, Vec<(String, Option<Value>)>> = BTreeMap::new();
        for write in writes {
            domains
                .entry(write.id.domain)
                .or_default()
                .push((write.id.name, write.value));
        }
        let local = domains.keys().any(|domain| !self.hosts(domain));
        let layers = local.then(Layers::resolve);
        let mut failures = Vec::new();
        for (domain, writes) in domains {
            let failed = |name: &str, reason: &str| format!("{domain}/{name}: {reason}");
            let reasons: Vec<(String, String)> = match (self.seam, &layers) {
                (Some(seam), _) if self.hosts(&domain) => hosted(seam, &domain, writes),
                (_, Some(Ok(layers))) => stored(layers, &domain, writes),
                (_, Some(Err(error))) => {
                    let reason = error.to_string();
                    writes
                        .into_iter()
                        .map(|(name, _)| (name, reason.clone()))
                        .collect()
                }
                (_, None) => Vec::new(),
            };
            failures.extend(reasons.iter().map(|(name, reason)| failed(name, reason)));
        }
        if local {
            rikka_settings::flush_pending_writes();
        }
        failures
    }

    fn hosts(&self, domain: &str) -> bool {
        self.seam.is_some() && self.hosted.iter().any(|hosted| hosted == domain)
    }

    pub(crate) fn settle(&self, held: &[Held]) -> Result<(), String> {
        if let Some(seam) = self.seam {
            seam.call(protocol::reload())
                .blocking_wait()
                .map_err(|error| error.to_string())?;
        }
        let survey = survey()?;
        let repeats = held
            .iter()
            .filter(|held| {
                survey.find(&held.id).is_some_and(|setting| {
                    setting.user.as_ref() == Some(&held.value)
                        && setting.beneath == held.value
                        && setting.beneath_source == Source::Declared(Scope::User)
                })
            })
            .map(|held| Write {
                id: held.id.clone(),
                value: None,
            })
            .collect();
        match self.write_all(repeats).first() {
            Some(failure) => Err(failure.clone()),
            None => Ok(()),
        }
    }
}

fn hosted(
    seam: &SeamClient,
    domain: &str,
    writes: Vec<(String, Option<Value>)>,
) -> Vec<(String, String)> {
    writes
        .into_iter()
        .filter_map(|(name, value)| {
            let request = match value {
                Some(value) => protocol::set(domain, &name, value),
                None => protocol::reset(domain, &name),
            };
            let error = seam.call(request).blocking_wait().err()?;
            Some((name, error.to_string()))
        })
        .collect()
}

fn stored(
    layers: &Layers,
    domain: &str,
    writes: Vec<(String, Option<Value>)>,
) -> Vec<(String, String)> {
    let opened = catalog().and_then(|catalog| {
        let entry = catalog
            .entry(domain)
            .ok_or_else(|| "not a catalogued domain".to_owned())?;
        entry
            .open(layers.clone())
            .map_err(|error| error.to_string())
    });
    let mut store = match opened {
        Ok((store, _)) => store,
        Err(reason) => {
            return writes
                .into_iter()
                .map(|(name, _)| (name, reason.clone()))
                .collect();
        }
    };
    writes
        .into_iter()
        .filter_map(|(name, value)| {
            let error = match value {
                Some(value) => store.set_value(&name, value),
                None => store.reset(&name),
            }
            .err()?;
            Some((name, error.to_string()))
        })
        .collect()
}
