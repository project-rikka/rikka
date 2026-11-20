use rikka_llm::wire::random;
use rikka_llm_providers::Backend;

const ENDPOINT: &str = "endpoint:";
const DIGITS: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccountId(Kind);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Kind {
    Backend(Backend),
    Endpoint(u64),
}

impl AccountId {
    #[must_use]
    pub fn from_id(id: &str) -> Option<AccountId> {
        match id.strip_prefix(ENDPOINT) {
            Some(number) => AccountId::from_endpoint_number(number),
            None => Backend::from_id(id).map(AccountId::from),
        }
    }

    pub(crate) fn from_endpoint_number(number: &str) -> Option<AccountId> {
        (number.len() == DIGITS)
            .then(|| u64::from_str_radix(number, 16).ok())
            .flatten()
            .map(|number| AccountId(Kind::Endpoint(number)))
    }

    #[must_use]
    pub fn backend(self) -> Option<Backend> {
        match self.0 {
            Kind::Backend(backend) => Some(backend),
            Kind::Endpoint(_) => None,
        }
    }

    #[must_use]
    pub fn is_endpoint(self) -> bool {
        matches!(self.0, Kind::Endpoint(_))
    }

    pub(crate) fn minted() -> AccountId {
        AccountId(Kind::Endpoint(u64::from_be_bytes(random::bytes::<8>())))
    }

    pub(crate) fn kind(self) -> Kind {
        self.0
    }
}

pub(crate) fn hex(number: u64) -> String {
    format!("{number:016x}")
}

impl From<Backend> for AccountId {
    fn from(backend: Backend) -> AccountId {
        AccountId(Kind::Backend(backend))
    }
}

impl std::fmt::Display for AccountId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Kind::Backend(backend) => formatter.write_str(backend.id()),
            Kind::Endpoint(number) => write!(formatter, "{ENDPOINT}{}", hex(number)),
        }
    }
}

impl std::fmt::Debug for AccountId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("AccountId")
            .field(&self.to_string())
            .finish()
    }
}

impl serde::Serialize for AccountId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for AccountId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<AccountId, D::Error> {
        let id = String::deserialize(deserializer)?;
        AccountId::from_id(&id)
            .ok_or_else(|| serde::de::Error::custom(format!("no account is called \"{id}\"")))
    }
}
