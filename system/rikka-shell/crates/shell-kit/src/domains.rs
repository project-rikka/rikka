use std::cell::RefCell;
use std::rc::Rc;

use rikka_seam::{CapabilityId, Delivery, SeamClient};
use rikka_settings::Schema;
use rikka_settings::seam::{CAPABILITY, Changed, Settings, protocol};
use serde::de::DeserializeOwned;

use crate::host::Host;

pub(crate) type Reader = Rc<RefCell<Settings>>;

type Act = Box<dyn Fn(&mut Host, &Settings)>;

struct Follower {
    key: Option<&'static str>,
    act: Act,
}

struct Domain {
    settings: Reader,
    followers: Vec<Follower>,
}

#[derive(Default)]
pub struct Domains(Vec<Domain>);

impl Domains {
    pub(super) fn declare(&mut self, schema: Schema, session: Option<SeamClient>) {
        let settings_reader = match session {
            Some(seam) => Settings::with_seam(schema, seam),
            None => Settings::local(schema),
        };
        let Ok(settings) = settings_reader.inspect_err(|error| {
            tracing::warn!("failed to read settings domain: {error}");
        }) else {
            return;
        };
        let domain = settings.domain();
        if settings.is_shared() {
            tracing::info!("declared settings domain `{domain}`");
        } else {
            tracing::warn!("settings domain `{domain}` is local only");
        }
        self.0.push(Domain {
            settings: Rc::new(RefCell::new(settings)),
            followers: Vec::new(),
        });
    }

    pub(crate) fn get<T: DeserializeOwned>(&self, domain: &str, key: &str) -> Option<T> {
        let Some(settings) = self.reader(domain) else {
            tracing::warn!("`{domain}/{key}` read before domain was declared");
            return None;
        };
        settings.borrow().read(key)
    }

    pub(crate) fn reader(&self, domain: &str) -> Option<Reader> {
        self.find(domain).map(|found| Rc::clone(&found.settings))
    }

    pub(super) fn follow(&mut self, domain: &str, key: Option<&'static str>, act: Act) {
        let Some(found) = self.0.iter_mut().find(|found| found.is(domain)) else {
            return tracing::warn!("`{domain}` followed before it was declared");
        };
        found.followers.push(Follower { key, act });
    }

    #[must_use]
    pub fn is_followed(&self) -> bool {
        self.0.iter().any(|domain| !domain.followers.is_empty())
    }

    pub fn deliver(&self, host: &mut Host, incoming: &Delivery) {
        match incoming {
            Delivery::Event(event) => {
                if event.capability().as_str() != CAPABILITY || event.name() != protocol::CHANGED {
                    return;
                }
                match event.decode::<Changed>() {
                    Ok(changed) => self.changed(host, &changed),
                    Err(error) => tracing::warn!("malformed settings announcement: {error}"),
                }
            }
            Delivery::Lagged(lagged) => {
                if lagged.includes(&CapabilityId::from(CAPABILITY)) {
                    self.0.iter().for_each(|domain| domain.reread(host));
                }
            }
        }
    }

    fn changed(&self, host: &mut Host, changed: &Changed) {
        let Some(found) = self.find(changed.domain.as_str()) else {
            return;
        };
        found.settings.borrow_mut().apply_change(changed);
        let settings = found.settings.borrow();
        for follower in &found.followers {
            if follower.key.is_none_or(|key| changed.changes_key(key)) {
                (follower.act)(host, &settings);
            }
        }
    }

    fn find(&self, domain: &str) -> Option<&Domain> {
        self.0.iter().find(|found| found.is(domain))
    }
}

impl Domain {
    fn is(&self, domain: &str) -> bool {
        self.settings.borrow().domain().as_str() == domain
    }

    fn reread(&self, host: &mut Host) {
        if self.followers.is_empty() {
            return;
        }
        if let Err(error) = self.settings.borrow_mut().refresh() {
            let domain = self.settings.borrow().domain().clone();
            return tracing::warn!("failed to reload settings domain `{domain}`: {error}");
        }
        let settings = self.settings.borrow();
        for follower in &self.followers {
            (follower.act)(host, &settings);
        }
    }
}

impl std::fmt::Debug for Domains {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.0.iter().map(|domain| {
                (
                    domain.settings.borrow().domain().clone(),
                    domain.followers.len(),
                )
            }))
            .finish()
    }
}
