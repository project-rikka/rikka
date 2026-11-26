use std::collections::BTreeMap;

use rikka_seam::{
    Call, EventInfo, Manifest, Method, MethodInfo, SeamError, Service, ServiceContext,
};

use crate::domain::Domain;
use crate::error::Error;
use crate::layers::Layers;
use crate::schema::Schema;
use crate::seam::protocol::{
    self, CAPABILITY, Changed, DomainRef, DomainRequest, KeyRef, SetRequest, VERSION,
};
use crate::store::Store;

#[derive(Debug)]
pub struct SettingsService {
    layers: Layers,
    stores: BTreeMap<Domain, Entry>,
    seam: Option<ServiceContext>,
}

#[derive(Debug)]
struct Entry {
    store: Store,
    locked: bool,
}

impl SettingsService {
    pub fn new() -> Result<Self, Error> {
        Ok(Self::with_layers(Layers::resolve()?))
    }

    #[must_use]
    pub fn with_layers(layers: Layers) -> Self {
        Self {
            layers,
            stores: BTreeMap::new(),
            seam: None,
        }
    }

    pub fn with_schema(mut self, schema: Schema) -> Result<Self, Error> {
        self.install(schema, true)?;
        Ok(self)
    }

    fn install(&mut self, schema: Schema, locked: bool) -> Result<Domain, Error> {
        let (store, report) = Store::load(schema, self.layers.clone())?;
        report.log_warnings(store.domain());
        let domain = store.domain().clone();
        self.stores.insert(domain.clone(), Entry { store, locked });
        Ok(domain)
    }

    fn entry(&mut self, domain: &Domain) -> Result<&mut Entry, SeamError> {
        self.stores
            .get_mut(domain)
            .ok_or_else(|| undeclared(domain))
    }

    fn declare(&mut self, call: Call) {
        let schema = match call.decode::<Schema>() {
            Ok(schema) => schema,
            Err(error) => return call.fail(error),
        };
        let domain = match schema.validate() {
            Ok(domain) => domain,
            Err(error) => return call.fail(SeamError::invalid_params(error)),
        };
        if let Some(entry) = self.stores.get(&domain) {
            if entry.locked {
                return match disagreement(entry.store.schema(), &schema) {
                    None => call.done(),
                    Some(reason) => call.fail(SeamError::unavailable(format!(
                        "`{domain}` belongs to the desktop: {reason}"
                    ))),
                };
            }
            if entry.store.schema() == &schema {
                return call.done();
            }
        }
        match self.install(schema, false) {
            Ok(_) => call.done(),
            Err(error) => call.fail(SeamError::failed(error)),
        }
    }

    fn answer<P, T>(&self, call: Call, read: impl FnOnce(&Store, &P) -> Result<T, SeamError>)
    where
        P: serde::de::DeserializeOwned + DomainRequest,
        T: serde::Serialize,
    {
        let request = match call.decode::<P>() {
            Ok(request) => request,
            Err(error) => return call.fail(error),
        };
        let answer = self
            .stores
            .get(request.domain())
            .ok_or_else(|| undeclared(request.domain()))
            .and_then(|entry| read(&entry.store, &request));
        match answer {
            Ok(answer) => call.encode(&answer),
            Err(error) => call.fail(error),
        }
    }

    fn set(&mut self, call: Call) {
        let request = match call.decode::<SetRequest>() {
            Ok(request) => request,
            Err(error) => return call.fail(error),
        };
        self.write(call, request.domain, |store| {
            store.set_value(&request.key, request.value)
        });
    }

    fn reset(&mut self, call: Call) {
        let request = match call.decode::<KeyRef>() {
            Ok(request) => request,
            Err(error) => return call.fail(error),
        };
        self.write(call, request.domain, |store| store.reset(&request.key));
    }

    fn write(
        &mut self,
        call: Call,
        domain: Domain,
        change: impl FnOnce(&mut Store) -> Result<(), Error>,
    ) {
        let entry = match self.entry(&domain) {
            Ok(entry) => entry,
            Err(error) => return call.fail(error),
        };

        let before = entry.store.snapshot();
        let changed = change(&mut entry.store);
        let changes = before.diff(&entry.store.snapshot());
        let values = entry.store.user_values().clone();

        if !changes.is_empty()
            && let Some(seam) = &self.seam
            && let Err(error) = seam.publisher().emit(
                protocol::CHANGED,
                &Changed {
                    domain,
                    changes,
                    user_values: values.clone(),
                    reread: false,
                },
            )
        {
            return call.fail(error);
        }

        match changed {
            Ok(()) => call.encode(&values),
            Err(error) => call.fail(refused(error)),
        }
    }

    fn reload(&mut self, call: Call) {
        let mut moved = Vec::new();
        for (domain, entry) in &mut self.stores {
            let before = entry.store.snapshot();
            let sources = entry.store.sources();
            match entry.store.reload() {
                Ok(report) => report.log_warnings(domain),
                Err(error) => {
                    tracing::warn!("failed to reload settings domain `{domain}`: {error}");
                    continue;
                }
            }
            let changes = before.diff(&entry.store.snapshot());
            if changes.is_empty() && sources == entry.store.sources() {
                continue;
            }
            moved.push(Changed {
                domain: domain.clone(),
                changes,
                user_values: entry.store.user_values().clone(),
                reread: true,
            });
        }
        if let Some(seam) = &self.seam {
            for changed in &moved {
                if let Err(error) = seam.publisher().emit(protocol::CHANGED, changed) {
                    return call.fail(error);
                }
            }
        }
        let domains: Vec<&Domain> = moved.iter().map(|changed| &changed.domain).collect();
        call.encode(&domains);
    }
}

fn undeclared(domain: &Domain) -> SeamError {
    SeamError::invalid_params(format!("`{domain}` is not a domain anything has declared"))
}

fn disagreement(current: &Schema, proposed: &Schema) -> Option<String> {
    for key in proposed.keys() {
        match current.key_named(key.name()) {
            Some(mine) if mine == key => {}
            Some(_) => return Some(format!("`{}` is not what it says it is", key.name())),
            None => return Some(format!("it declares no `{}`", key.name())),
        }
    }
    None
}

fn refused(error: Error) -> SeamError {
    match error {
        Error::UnknownKey { .. }
        | Error::Refused { .. }
        | Error::Locked { .. }
        | Error::Convert(_) => SeamError::invalid_params(error),
        other => SeamError::failed(other),
    }
}

impl<H> Service<H> for SettingsService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, VERSION)
            .summary("Every setting the desktop and its applications keep")
            .method(MethodInfo::new(protocol::DOMAINS).summary("Every domain that is declared"))
            .method(
                MethodInfo::new(protocol::DESCRIBE)
                    .summary("The schema of `domain` — its settings, their types and their bounds"),
            )
            .method(
                MethodInfo::new(protocol::DECLARE)
                    .summary("Adds a domain, or replaces the schema of one already there"),
            )
            .method(MethodInfo::new(protocol::READ).summary("Every setting in `domain`, resolved"))
            .method(MethodInfo::new(protocol::GET).summary("What `domain`'s `key` currently is"))
            .method(
                MethodInfo::new(protocol::USER_VALUES)
                    .summary("Everything the user has said in `domain`, as the writer holds it"),
            )
            .method(MethodInfo::new(protocol::SET).summary("Writes `domain`'s `key` to `value`"))
            .method(
                MethodInfo::new(protocol::RESET)
                    .summary("Takes back whatever the user said about `domain`'s `key`"),
            )
            .method(
                MethodInfo::new(protocol::SOURCE)
                    .summary("Which layer answered for `domain`'s `key`"),
            )
            .method(
                MethodInfo::new(protocol::SOURCES)
                    .summary("Which layer answered for each of `domain`'s settings"),
            )
            .method(MethodInfo::new(protocol::RELOAD).summary(
                "Reads every domain's files afresh after the system configuration changed",
            ))
            .event(
                EventInfo::new(protocol::CHANGED).summary(
                    "A write moved something, with everything it moved and everything held",
                ),
            )
    }

    fn attach(&mut self, seam: ServiceContext) {
        self.seam = Some(seam);
    }

    fn call(&mut self, _: &mut H, method: Method, call: Call) {
        match &*method {
            protocol::DOMAINS => {
                let domains: Vec<&Domain> = self.stores.keys().collect();
                call.encode(&domains);
            }
            protocol::DESCRIBE => {
                self.answer(call, |store, _: &DomainRef| Ok(store.schema().clone()));
            }
            protocol::DECLARE => self.declare(call),
            protocol::READ => {
                self.answer(call, |store, _: &DomainRef| {
                    Ok(store.snapshot().values().clone())
                });
            }
            protocol::GET => {
                self.answer(call, |store, request: &KeyRef| {
                    store.value(&request.key).cloned().map_err(refused)
                });
            }
            protocol::USER_VALUES => {
                self.answer(call, |store, _: &DomainRef| Ok(store.user_values().clone()));
            }
            protocol::SET => self.set(call),
            protocol::RESET => self.reset(call),
            protocol::SOURCE => {
                self.answer(call, |store, request: &KeyRef| {
                    store.source(&request.key).map_err(refused)
                });
            }
            protocol::SOURCES => {
                self.answer(call, |store, _: &DomainRef| Ok(store.sources()));
            }
            protocol::RELOAD => self.reload(call),
            _ => call.unknown_method(),
        }
    }
}
