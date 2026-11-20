use std::collections::HashMap;
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use music_core::{Error, Result};
use music_stream::{Scan, answer_text, is_word};
use rikka_net::Client;
use serde::{Deserialize, Serialize};

const LOGIN_PAGE: &str = "https://play.qobuz.com/login";
const PLAYER: &str = "https://play.qobuz.com";
const KEPT: &str = "bundle.json";
const CHECKSUM: usize = 44;
const APP_ID_DIGITS: usize = 9;
const SECRET_DIGITS: usize = 32;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct App {
    version: String,
    pub(crate) id: String,
    pub(crate) secrets: Vec<String>,
    pub(crate) private_key: Option<String>,
}

impl App {
    pub(crate) async fn current(http: &Client, folder: &Path, fresh: bool) -> Result<App> {
        let cached = (!fresh).then(|| load_cached(folder)).flatten();
        let page = match answer_text(http.get(LOGIN_PAGE)).await {
            Ok(page) => page,
            Err(offline) => return cached.ok_or(offline),
        };
        let (path, version) = bundle_path(&page)?;
        if let Some(cached) = cached.filter(|cached| cached.version == version) {
            return Ok(cached);
        }
        let bundle = answer_text(http.get(format!("{PLAYER}{path}"))).await?;
        let app = App::extracted(&bundle, version)?;
        std::fs::create_dir_all(folder).map_err(|error| Error::Storage(error.to_string()))?;
        rikka_account_book::write_private_json(&folder.join(KEPT), &app)?;
        Ok(app)
    }

    fn extracted(bundle: &str, version: String) -> Result<App> {
        let id = Scan::new(bundle)
            .matches(r#"production:{api:{appId:""#, |mut scan| {
                let id = scan.take(APP_ID_DIGITS..=APP_ID_DIGITS, |c| c.is_ascii_digit())?;
                scan.expect("\"")?;
                Some(id.to_owned())
            })
            .next()
            .ok_or_else(|| Error::Unreadable("the Qobuz player has no app id".into()))?;
        let mut secrets = seeded_secrets(bundle);
        if secrets.is_empty() {
            secrets = Scan::new(bundle)
                .matches(r#"appSecret:""#, |mut scan| {
                    let secret = scan.take(SECRET_DIGITS..=SECRET_DIGITS, |c| {
                        c.is_ascii_digit() || ('a'..='f').contains(&c)
                    })?;
                    scan.expect("\"")?;
                    Some(secret.to_owned())
                })
                .collect();
        }
        if secrets.is_empty() {
            return Err(Error::Unreadable(
                "the Qobuz player has no app secret".into(),
            ));
        }
        Ok(App {
            version,
            id,
            secrets,
            private_key: private_key(bundle),
        })
    }
}

impl std::fmt::Debug for App {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("App")
            .field("version", &self.version)
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

fn load_cached(folder: &Path) -> Option<App> {
    let bytes = std::fs::read(folder.join(KEPT)).ok()?;
    serde_json::from_slice::<App>(&bytes)
        .ok()
        .filter(|app| !app.id.is_empty() && !app.secrets.is_empty())
}

fn bundle_path(page: &str) -> Result<(String, String)> {
    Scan::new(page)
        .matches(r#"<script src="/resources/"#, |mut scan| {
            let version = scan.take(1..=usize::MAX, |c| {
                c.is_ascii_alphanumeric() || c == '.' || c == '-'
            })?;
            scan.expect(r#"/bundle.js"></script>"#)?;
            Some((
                format!("/resources/{version}/bundle.js"),
                version.to_owned(),
            ))
        })
        .next()
        .ok_or_else(|| Error::Unreadable("the Qobuz sign-in page has no player link".into()))
}

fn private_key(bundle: &str) -> Option<String> {
    Scan::new(bundle)
        .matches("privateKey:", |mut scan| {
            scan.skip_while(char::is_whitespace);
            scan.expect("\"")?;
            let key = scan.take(6..=30, |c| c.is_ascii_alphanumeric())?;
            scan.expect("\"")?;
            Some(key.to_owned())
        })
        .next()
}

fn seeded_secrets(bundle: &str) -> Vec<String> {
    let seeded = |c: char| is_word(c) || c == '=';
    let seeds: HashMap<String, String> = Scan::new(bundle)
        .matches(r#".initialSeed(""#, |mut scan| {
            let seed = scan.take(1..=usize::MAX, seeded)?;
            scan.expect(r#"",window.utimezone."#)?;
            let zone = scan.take(1..=usize::MAX, |c| c.is_ascii_lowercase())?;
            scan.expect(")")?;
            Some((zone.to_owned(), seed.to_owned()))
        })
        .collect();
    Scan::new(bundle)
        .matches(r#"name:""#, |mut scan| {
            scan.take(1..=usize::MAX, is_word)?;
            scan.expect("/")?;
            let city = scan.take(2..=usize::MAX, |c| c.is_ascii_alphabetic())?;
            let capitalised = city.starts_with(|c: char| c.is_ascii_uppercase())
                && city[1..].chars().all(|c| c.is_ascii_lowercase());
            if !capitalised {
                return None;
            }
            scan.expect(r#"",info:""#)?;
            let info = scan.take(1..=usize::MAX, seeded)?;
            scan.expect(r#"",extras:""#)?;
            let extras = scan.take(1..=usize::MAX, seeded)?;
            scan.expect("\"")?;
            Some((city, info, extras))
        })
        .filter_map(|(city, info, extras)| {
            let seed = seeds.get(&city.to_ascii_lowercase())?;
            let joined = format!("{seed}{info}{extras}");
            let encoded = joined.get(..joined.len().checked_sub(CHECKSUM)?)?;
            let decoded = URL_SAFE_NO_PAD.decode(encoded.trim_end_matches('=')).ok()?;
            String::from_utf8(decoded).ok()
        })
        .collect()
}
