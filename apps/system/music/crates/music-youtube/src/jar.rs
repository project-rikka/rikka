use rikka_sync::{Mutex, MutexGuard};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use music_core::{Cookie, Error, Result, keep_secret, kept_secret, renew_secret};
use sha1::{Digest, Sha1};

pub(crate) const PROOF: &[&str] = &["SAPISID", "__Secure-3PAPISID"];
const KIND: &str = "cookies";
const LABEL: &str = "YouTube Music sign-in";
const SECONDS_PER_YEAR: u64 = 31_556_952;

type Cookies = BTreeMap<String, String>;

pub(crate) struct Jar {
    folder: PathBuf,
    cookies: Mutex<Cookies>,
}

impl Jar {
    pub(crate) async fn fill(folder: &Path, cookies: &[Cookie]) -> Result<Jar> {
        let saved: Cookies = cookies
            .iter()
            .filter(|cookie| !cookie.name.is_empty())
            .map(|cookie| (cookie.name.clone(), cookie.value.clone()))
            .collect();
        if !PROOF.iter().any(|name| saved.contains_key(*name)) {
            return Err(Error::Refused("Google returned no sign-in cookies".into()));
        }
        keep_secret(folder, KIND, LABEL, &saved).await?;
        Ok(Jar {
            folder: folder.to_owned(),
            cookies: Mutex::new(saved),
        })
    }

    pub(crate) async fn open(folder: &Path) -> Result<Option<Jar>> {
        Ok(kept_secret(folder, KIND).await?.map(|cookies| Jar {
            folder: folder.to_owned(),
            cookies: Mutex::new(cookies),
        }))
    }

    pub(crate) fn header(&self) -> String {
        self.lock()
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("; ")
    }

    pub(crate) fn authorization(&self, origin: &str) -> Option<String> {
        let cookies = self.lock();
        let secret = PROOF.iter().find_map(|name| cookies.get(*name))?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        Some(format!("SAPISIDHASH {}", hash(now, secret, origin)))
    }

    pub(crate) async fn absorb(&self, set: Vec<String>) {
        let this_year = this_year();
        let changed = {
            let mut cookies = self.lock();
            let mut changed = false;
            for line in &set {
                changed |= match parse(line, this_year) {
                    Some(Set::Keep(name, value)) => {
                        cookies.insert(name, value.clone()).as_ref() != Some(&value)
                    }
                    Some(Set::Drop(name)) => cookies.remove(&name).is_some(),
                    None => false,
                };
            }
            changed.then(|| cookies.clone())
        };
        if let Some(cookies) = changed
            && let Err(error) = renew_secret(&self.folder, KIND, LABEL, &cookies).await
        {
            tracing::warn!("failed to save YouTube cookies: {error}");
        }
    }

    fn lock(&self) -> MutexGuard<'_, Cookies> {
        self.cookies.lock()
    }
}

impl std::fmt::Debug for Jar {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Jar")
            .field("folder", &self.folder)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Set {
    Keep(String, String),
    Drop(String),
}

fn parse(line: &str, this_year: u64) -> Option<Set> {
    let mut parts = line.split(';').map(str::trim);
    let (name, value) = parts.next()?.split_once('=')?;
    if name.is_empty() {
        return None;
    }
    let expired = parts.any(|attribute| {
        let (key, setting) = attribute.split_once('=').unwrap_or((attribute, ""));
        match key.to_ascii_lowercase().as_str() {
            "max-age" => setting.trim().parse::<i64>().is_ok_and(|age| age <= 0),
            "expires" => year(setting).is_some_and(|year| year < this_year),
            _ => false,
        }
    });
    Some(match expired {
        true => Set::Drop(name.to_owned()),
        false => Set::Keep(name.to_owned(), value.to_owned()),
    })
}

fn year(date: &str) -> Option<u64> {
    date.split(|character: char| !character.is_ascii_digit())
        .find(|token| token.len() == 4)
        .and_then(|token| token.parse().ok())
}

fn this_year() -> u64 {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    1970 + seconds / SECONDS_PER_YEAR
}

fn hash(now: u64, secret: &str, origin: &str) -> String {
    let digest = Sha1::digest(format!("{now} {secret} {origin}").as_bytes());
    let hex = rikka_types::hex::encode(digest);
    format!("{now}_{hex}")
}
