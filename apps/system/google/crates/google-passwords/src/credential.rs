use google_sync::proto::{Note, Notes, PasswordSpecificsData};
use google_sync::{unix_ms_from_windows_micros, windows_micros_from_unix_ms};
use rikka_net::Url;

use crate::error::{Error, Result};

const ANDROID: &str = "android";
const WEB: [&str; 2] = ["http", "https"];
const MANUALLY_ADDED: i32 = 2;
const HTML_SCHEME: i32 = 0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    pub realm: String,
    pub address: String,
    pub name: String,
    pub group: String,
    pub is_app: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Credential {
    pub id: String,
    pub site: Site,
    pub username: String,
    pub password: String,
    pub note: String,
    pub federation: Option<String>,
    pub created_ms: Option<i64>,
    pub used_ms: Option<i64>,
    pub changed_ms: Option<i64>,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Draft {
    pub address: String,
    pub username: String,
    pub password: String,
    pub note: String,
}

impl Site {
    fn new(realm: &str, origin: &str) -> Site {
        let parsed = Url::parse(realm).ok();
        let is_app = parsed.as_ref().is_some_and(|url| url.scheme() == ANDROID);
        let host = parsed
            .as_ref()
            .and_then(Url::host_str)
            .unwrap_or(realm)
            .to_owned();
        let name = host.strip_prefix("www.").unwrap_or(&host).to_owned();
        let is_domain = parsed.as_ref().is_some_and(|url| url.domain().is_some());
        let group = match is_domain && !is_app {
            true => rikka_public_suffix::site(&name).to_owned(),
            false => name.clone(),
        };
        let opens = |address: &str| {
            Url::parse(address)
                .ok()
                .is_some_and(|url| WEB.contains(&url.scheme()))
        };
        let address = match (opens(origin), opens(realm)) {
            (true, _) => origin.to_owned(),
            (false, true) => realm.to_owned(),
            (false, false) => String::new(),
        };
        Site {
            realm: realm.to_owned(),
            address,
            name,
            group,
            is_app,
        }
    }
}

impl Credential {
    #[must_use]
    pub fn from_specifics(id: &str, data: &PasswordSpecificsData) -> Option<Credential> {
        if data.blacklisted == Some(true) {
            return None;
        }
        let realm = data
            .signon_realm
            .as_deref()
            .filter(|realm| !realm.is_empty())?;
        let time = |micros: Option<i64>| {
            micros
                .filter(|micros| *micros > 0)
                .map(unix_ms_from_windows_micros)
        };
        Some(Credential {
            id: id.to_owned(),
            site: Site::new(realm, data.origin.as_deref().unwrap_or_default()),
            username: data.username_value.clone().unwrap_or_default(),
            password: data.password_value.clone().unwrap_or_default(),
            note: data
                .notes
                .as_ref()
                .and_then(|notes| notes.note.iter().find(|note| is_plain_note(note)))
                .and_then(|note| note.value.clone())
                .unwrap_or_default(),
            federation: data
                .federation_url
                .as_deref()
                .filter(|federation| !federation.is_empty())
                .and_then(|federation| Url::parse(federation).ok())
                .and_then(|federation| federation.host_str().map(str::to_owned)),
            created_ms: time(data.date_created),
            used_ms: time(data.date_last_used),
            changed_ms: time(data.date_password_modified_windows_epoch_micros),
        })
    }

    #[must_use]
    pub fn matches(&self, folded_needle: &str) -> bool {
        folded_needle.is_empty()
            || [&self.site.name, &self.site.realm, &self.username]
                .iter()
                .any(|field| field.to_lowercase().contains(folded_needle))
    }
}

impl Draft {
    #[must_use]
    pub fn from_credential(credential: &Credential) -> Draft {
        Draft {
            address: credential.site.address.clone(),
            username: credential.username.clone(),
            password: credential.password.clone(),
            note: credential.note.clone(),
        }
    }

    pub(crate) fn specifics(
        &self,
        base: Option<&PasswordSpecificsData>,
        now_ms: i64,
    ) -> Result<PasswordSpecificsData> {
        if self.password.is_empty() && base.is_none_or(|base| base.federation_url.is_none()) {
            return Err(Error::EmptyPassword);
        }
        let now = windows_micros_from_unix_ms(now_ms);
        let mut data = match base {
            Some(base) => base.clone(),
            None => {
                let address = web_address(&self.address)?;
                PasswordSpecificsData {
                    scheme: Some(HTML_SCHEME),
                    signon_realm: Some(format!("{}/", address.origin().ascii_serialization())),
                    origin: Some(address.into()),
                    date_created: Some(now),
                    blacklisted: Some(false),
                    r#type: Some(MANUALLY_ADDED),
                    times_used: Some(0),
                    ..PasswordSpecificsData::default()
                }
            }
        };
        if data.password_value.as_deref() != Some(self.password.as_str()) {
            data.password_value = Some(self.password.clone());
            data.date_password_modified_windows_epoch_micros = Some(now);
            data.password_issues = None;
        }
        data.username_value = Some(self.username.trim().to_owned());
        self.write_note(&mut data, now);
        Ok(data)
    }

    fn write_note(&self, data: &mut PasswordSpecificsData, now: i64) {
        let notes = data.notes.get_or_insert_with(Notes::default);
        match notes.note.iter_mut().find(|note| is_plain_note(note)) {
            Some(note) => note.value = Some(self.note.clone()),
            None if self.note.is_empty() => {}
            None => notes.note.push(Note {
                unique_display_name: Some(String::new()),
                value: Some(self.note.clone()),
                date_created_windows_epoch_micros: Some(now),
                hide_by_default: Some(false),
            }),
        }
        if notes.note.is_empty() {
            data.notes = None;
        }
    }
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Credential")
            .field("id", &self.id)
            .field("site", &self.site.name)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Draft {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Draft")
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}

fn is_plain_note(note: &Note) -> bool {
    note.unique_display_name
        .as_deref()
        .is_none_or(str::is_empty)
}

fn web_address(typed: &str) -> Result<Url> {
    let typed = typed.trim();
    let address = match typed.contains("://") {
        true => typed.to_owned(),
        false => format!("https://{typed}"),
    };
    Url::parse(&address)
        .ok()
        .filter(|url| WEB.contains(&url.scheme()))
        .filter(|url| url.host_str().is_some_and(|host| !host.is_empty()))
        .ok_or(Error::Address)
}
