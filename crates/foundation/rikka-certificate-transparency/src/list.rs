use std::collections::HashMap;
use std::sync::Arc;

use base64::prelude::{BASE64_STANDARD, Engine as _};
use jiff::{SignedDuration, Timestamp};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};

use crate::error::{Error, Result};
use crate::key::LogListKey;
use crate::log::{Log, LogId, LogState};
use crate::public_key::PublicKey;

const MAX_AGE: SignedDuration = SignedDuration::from_hours(70 * 24);

#[derive(Debug, Clone)]
pub struct LogList {
    version: String,
    timestamp: Timestamp,
    expires_at: Timestamp,
    logs: HashMap<LogId, Log>,
}

impl LogList {
    pub fn from_signed(list: &[u8], signature: &[u8], key: &LogListKey) -> Result<LogList> {
        if !key.verifies(list, signature) {
            return Err(Error::Signature);
        }
        LogList::parse(list)
    }

    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    #[must_use]
    pub fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    #[must_use]
    pub fn expires_at(&self) -> Timestamp {
        self.expires_at
    }

    #[must_use]
    pub fn is_fresh(&self, now: Timestamp) -> bool {
        now <= self.expires_at
    }

    pub fn logs(&self) -> impl Iterator<Item = &Log> {
        self.logs.values()
    }

    pub(crate) fn log(&self, id: &LogId) -> Option<&Log> {
        self.logs.get(id)
    }

    fn parse(list: &[u8]) -> Result<LogList> {
        let document: Document = serde_json::from_slice(list)
            .map_err(|error| Error::MalformedList(error.to_string()))?;
        let timestamp = parse_timestamp(&document.log_list_timestamp)?;
        let expires_at = timestamp
            .checked_add(MAX_AGE)
            .map_err(|error| Error::MalformedList(error.to_string()))?;

        let mut logs = HashMap::new();
        for operator in document.operators {
            let name: Arc<str> = operator.name.into();
            for listed in operator.logs.into_iter().chain(operator.tiled_logs) {
                let description = listed.description.clone();
                match parse_log(listed, &name) {
                    Some(log) => {
                        logs.insert(log.id, log);
                    }
                    None => tracing::warn!(log = %description, "skipping unusable CT log entry"),
                }
            }
        }

        Ok(LogList {
            version: document.version,
            timestamp,
            expires_at,
            logs,
        })
    }
}

#[derive(Deserialize)]
struct Document {
    version: String,
    log_list_timestamp: String,
    operators: Vec<Operator>,
}

#[derive(Deserialize)]
struct Operator {
    name: String,
    #[serde(default)]
    logs: Vec<ListedLog>,
    #[serde(default)]
    tiled_logs: Vec<ListedLog>,
}

#[derive(Deserialize)]
struct ListedLog {
    description: String,
    log_id: String,
    key: String,
    state: Option<ListedState>,
}

#[derive(Deserialize)]
struct ListedState {
    pending: Option<Transition>,
    qualified: Option<Transition>,
    usable: Option<Transition>,
    readonly: Option<Transition>,
    retired: Option<Transition>,
    rejected: Option<Transition>,
}

#[derive(Deserialize)]
struct Transition {
    timestamp: String,
}

fn parse_log(listed: ListedLog, operator: &Arc<str>) -> Option<Log> {
    let id: [u8; 32] = BASE64_STANDARD
        .decode(&listed.log_id)
        .ok()?
        .try_into()
        .ok()?;
    let key_der = BASE64_STANDARD.decode(&listed.key).ok()?;
    if Sha256::digest(&key_der).as_slice() != id {
        return None;
    }
    Some(Log {
        id: LogId::new(id),
        description: listed.description,
        operator: Arc::clone(operator),
        key: PublicKey::from_spki(&key_der)?,
        state: parse_state(listed.state?)?,
    })
}

fn parse_state(state: ListedState) -> Option<LogState> {
    let ListedState {
        pending,
        qualified,
        usable,
        readonly,
        retired,
        rejected,
    } = state;
    let states = [
        pending.map(|_| LogState::Pending),
        qualified.map(|_| LogState::Qualified),
        usable.map(|_| LogState::Usable),
        readonly.map(|_| LogState::ReadOnly),
        match retired {
            Some(transition) => Some(LogState::Retired(
                parse_timestamp(&transition.timestamp).ok()?,
            )),
            None => None,
        },
        rejected.map(|_| LogState::Rejected),
    ];
    let mut present = states.into_iter().flatten();
    let state = present.next()?;
    present.next().is_none().then_some(state)
}

fn parse_timestamp(timestamp: &str) -> Result<Timestamp> {
    timestamp
        .parse()
        .map_err(|error: jiff::Error| Error::MalformedList(format!("{timestamp}: {error}")))
}
