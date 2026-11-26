use rikka_seam::{Event, Request, Version};
use serde::Serialize;

use crate::domain::Domain;
use crate::schema::Schema;
use crate::snapshot::Change;
use crate::value::{Value, Values};

pub const CAPABILITY: &str = "rikka.settings";

pub const VERSION: Version = Version::new(1, 0);

pub const DOMAINS: &str = "domains";
pub const DESCRIBE: &str = "describe";
pub const DECLARE: &str = "declare";
pub const READ: &str = "read";
pub const GET: &str = "get";
pub const USER_VALUES: &str = "held";
pub const SET: &str = "set";
pub const RESET: &str = "reset";
pub const SOURCE: &str = "source";
pub const SOURCES: &str = "sources";
pub const RELOAD: &str = "reload";

pub const CHANGED: &str = "changed";

pub trait DomainRequest {
    fn domain(&self) -> &Domain;
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DomainRef {
    pub domain: Domain,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct KeyRef {
    pub domain: Domain,
    pub key: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SetRequest {
    pub domain: Domain,
    pub key: String,
    pub value: Value,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Changed {
    pub domain: Domain,
    pub changes: Vec<Change>,
    #[serde(rename = "held")]
    pub user_values: Values,
    #[serde(default)]
    pub reread: bool,
}

impl Changed {
    #[must_use]
    pub fn changes_key(&self, key: &str) -> bool {
        self.changes.iter().any(|change| change.key == key)
    }

    #[must_use]
    pub fn new_value(&self, key: &str) -> Option<Option<&Value>> {
        self.changes
            .iter()
            .find(|change| change.key == key)
            .map(|change| change.value.as_ref())
    }
}

#[derive(Serialize)]
struct Named<'a> {
    domain: &'a str,
}

#[derive(Serialize)]
struct Keyed<'a> {
    domain: &'a str,
    key: &'a str,
}

#[derive(Serialize)]
struct Setting<'a> {
    domain: &'a str,
    key: &'a str,
    value: Value,
}

fn asking(method: &str, body: &impl Serialize) -> Request {
    Request::new(CAPABILITY, method)
        .encode(body)
        .require(VERSION)
}

#[must_use]
pub fn domains() -> Request {
    asking(DOMAINS, &())
}

#[must_use]
pub fn declare(schema: &Schema) -> Request {
    asking(DECLARE, schema)
}

#[must_use]
pub fn describe(domain: &str) -> Request {
    asking(DESCRIBE, &Named { domain })
}

#[must_use]
pub fn read(domain: &str) -> Request {
    asking(READ, &Named { domain })
}

#[must_use]
pub fn user_values(domain: &str) -> Request {
    asking(USER_VALUES, &Named { domain })
}

#[must_use]
pub fn sources(domain: &str) -> Request {
    asking(SOURCES, &Named { domain })
}

#[must_use]
pub fn get(domain: &str, key: &str) -> Request {
    asking(GET, &Keyed { domain, key })
}

#[must_use]
pub fn source(domain: &str, key: &str) -> Request {
    asking(SOURCE, &Keyed { domain, key })
}

#[must_use]
pub fn set(domain: &str, key: &str, value: impl Into<Value>) -> Request {
    let value = value.into();
    asking(SET, &Setting { domain, key, value })
}

#[must_use]
pub fn reset(domain: &str, key: &str) -> Request {
    asking(RESET, &Keyed { domain, key })
}

#[must_use]
pub fn reload() -> Request {
    asking(RELOAD, &())
}

#[must_use]
pub fn changed(event: &Event, domain: &str) -> Option<Changed> {
    if event.capability().as_str() != CAPABILITY || event.name() != CHANGED {
        return None;
    }
    let changed = event.decode::<Changed>().ok()?;
    (changed.domain.as_str() == domain).then_some(changed)
}

impl DomainRequest for DomainRef {
    fn domain(&self) -> &Domain {
        &self.domain
    }
}

impl DomainRequest for KeyRef {
    fn domain(&self) -> &Domain {
        &self.domain
    }
}

impl DomainRequest for SetRequest {
    fn domain(&self) -> &Domain {
        &self.domain
    }
}
