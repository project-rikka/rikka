use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use zbus::zvariant::{OwnedValue, Type};

use rikka_dirs::passwd;

#[derive(Debug, Deserialize, Type)]
pub(crate) struct RawIdentity {
    kind: String,
    details: HashMap<String, OwnedValue>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct User {
    pub uid: u32,
    pub name: String,
    pub full_name: Option<String>,
}

pub(crate) fn users(identities: &[RawIdentity]) -> Vec<User> {
    let mut users = Vec::with_capacity(identities.len());
    for identity in identities {
        match identity.kind.as_str() {
            "unix-user" => match identity
                .details
                .get("uid")
                .and_then(|uid| u32::try_from(uid).ok())
            {
                Some(uid) => match lookup(uid) {
                    Some(user) => users.push(user),
                    None => tracing::warn!("unknown uid {uid} in polkit identity"),
                },
                None => tracing::warn!("`unix-user` identity has no valid uid"),
            },
            "unix-group" => {
                tracing::debug!("ignoring `unix-group` identity");
            }
            kind => tracing::debug!("unsupported polkit identity kind `{kind}`"),
        }
    }
    users
}

pub(crate) fn preferred(users: &[User]) -> usize {
    let own = passwd::current_uid();
    users.iter().position(|user| user.uid == own).unwrap_or(0)
}

fn lookup(uid: u32) -> Option<User> {
    let entry = passwd::lookup(uid)?;
    Some(User {
        uid,
        full_name: entry.full_name().map(str::to_owned),
        name: entry.name,
    })
}
