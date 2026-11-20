use rikka_secrets_protocol::names::{
    ALIAS_PREFIX, COLLECTION_PREFIX, NO_OBJECT, PROMPT_PREFIX, SESSION_PREFIX,
};
use zbus::zvariant::{ObjectPath, OwnedObjectPath};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Collection(String),
    Alias(String),
    Item(String, u64),
}

pub(crate) fn collection(name: &str) -> OwnedObjectPath {
    owned(format!("{COLLECTION_PREFIX}{name}"))
}

pub(crate) fn alias(name: &str) -> OwnedObjectPath {
    owned(format!("{ALIAS_PREFIX}{name}"))
}

pub(crate) fn item(collection: &str, id: u64) -> OwnedObjectPath {
    owned(format!("{COLLECTION_PREFIX}{collection}/{id}"))
}

pub(crate) fn session(serial: u64) -> OwnedObjectPath {
    owned(format!("{SESSION_PREFIX}s{serial}"))
}

pub(crate) fn prompt(serial: u64) -> OwnedObjectPath {
    owned(format!("{PROMPT_PREFIX}p{serial}"))
}

pub(crate) fn none() -> OwnedObjectPath {
    owned(NO_OBJECT.to_owned())
}

pub(crate) fn target(path: &ObjectPath<'_>) -> Option<Target> {
    let path = path.as_str();
    if let Some(name) = path.strip_prefix(ALIAS_PREFIX) {
        return is_element(name).then(|| Target::Alias(name.to_owned()));
    }
    let rest = path.strip_prefix(COLLECTION_PREFIX)?;
    match rest.split_once('/') {
        None => is_element(rest).then(|| Target::Collection(rest.to_owned())),
        Some((collection, id)) => {
            let id = id.parse().ok()?;
            is_element(collection).then(|| Target::Item(collection.to_owned(), id))
        }
    }
}

pub(crate) fn element(label: &str) -> String {
    let cleaned: String = label
        .chars()
        .map(|character| match character {
            'a'..='z' | '0'..='9' => character,
            'A'..='Z' => character.to_ascii_lowercase(),
            _ => '_',
        })
        .collect();
    let trimmed = cleaned.trim_matches('_');
    match trimmed.is_empty() {
        true => "collection".to_owned(),
        false => trimmed.to_owned(),
    }
}

pub(crate) fn is_element(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn owned(path: String) -> OwnedObjectPath {
    OwnedObjectPath::try_from(path).expect("keyring object paths are built from valid elements")
}
