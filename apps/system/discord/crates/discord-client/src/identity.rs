use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rand::RngCore;
use rikka_net::header;
use rikka_sync::RwLock;
use serde::{Deserialize, Serialize};

pub(crate) const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36";
pub(crate) const ORIGIN: &str = "https://discord.com";
pub(crate) const REFERER: &str = "https://discord.com/channels/@me";
const BROWSER_VERSION: &str = "142.0.0.0";
const FALLBACK_BUILD: u32 = 600_539;
const BUILD_FILE: &str = "build.json";
const BUILD_LASTS: Duration = Duration::from_secs(24 * 60 * 60);
const SENTRY_SCRIPT: &str = "/assets/sentry";
const SCRIPT_EXTENSION: &str = ".js";
const BUILD_NUMBER: &str = "buildNumber";
const BUILD_DIGITS_LEAST: usize = 5;
const BUILD_DIGITS_MOST: usize = 7;
const SIGNATURE_MASK: [u8; 16] = [
    0xff, 0x7f, 0xef, 0xef, 0xf7, 0xef, 0xf7, 0xff, 0xdf, 0x7e, 0xff, 0xbf, 0xfe, 0xff, 0xf7, 0xff,
];

#[derive(Debug)]
pub(crate) struct Identity {
    build: u32,
    launch_id: String,
    launch_signature: String,
    system_locale: String,
    locale: RwLock<String>,
    timezone: String,
    super_properties: String,
}

#[derive(Serialize)]
pub(crate) struct Properties<'a> {
    os: &'static str,
    browser: &'static str,
    device: &'static str,
    system_locale: &'a str,
    has_client_mods: bool,
    browser_user_agent: &'static str,
    browser_version: &'static str,
    os_version: &'static str,
    referrer: &'static str,
    referring_domain: &'static str,
    referrer_current: &'static str,
    referring_domain_current: &'static str,
    release_channel: &'static str,
    client_build_number: u32,
    client_event_source: Option<&'static str>,
    client_launch_id: &'a str,
    launch_signature: &'a str,
    client_app_state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_fast_connect: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gateway_connect_reasons: Option<&'static str>,
}

#[derive(Serialize, Deserialize)]
struct CachedBuild {
    number: u32,
    fetched: u64,
}

impl Identity {
    pub(crate) fn new(build: u32) -> Identity {
        let system_locale = system_locale();
        let mut identity = Identity {
            build,
            launch_id: random_uuid(None),
            launch_signature: random_uuid(Some(SIGNATURE_MASK)),
            locale: RwLock::new(discord_locale(&system_locale)),
            system_locale,
            timezone: jiff::tz::TimeZone::system()
                .iana_name()
                .unwrap_or("UTC")
                .to_owned(),
            super_properties: String::new(),
        };
        let json = serde_json::to_vec(&identity.properties(false)).unwrap_or_default();
        identity.super_properties = STANDARD.encode(json);
        identity
    }

    pub(crate) fn build(&self) -> u32 {
        self.build
    }

    pub(crate) fn properties(&self, gateway: bool) -> Properties<'_> {
        Properties {
            os: "Windows",
            browser: "Chrome",
            device: "",
            system_locale: &self.system_locale,
            has_client_mods: false,
            browser_user_agent: USER_AGENT,
            browser_version: BROWSER_VERSION,
            os_version: "10",
            referrer: "",
            referring_domain: "",
            referrer_current: "",
            referring_domain_current: "",
            release_channel: "stable",
            client_build_number: self.build,
            client_event_source: None,
            client_launch_id: &self.launch_id,
            launch_signature: &self.launch_signature,
            client_app_state: "focused",
            is_fast_connect: gateway.then_some(false),
            gateway_connect_reasons: gateway.then_some("AppSkeleton"),
        }
    }

    pub(crate) fn super_properties(&self) -> &str {
        &self.super_properties
    }

    pub(crate) fn locale(&self) -> String {
        self.locale.read().clone()
    }

    pub(crate) fn set_locale(&self, locale: &str) {
        if *self.locale.read() != locale {
            *self.locale.write() = locale.to_owned();
        }
    }

    pub(crate) fn headers(&self, referer: &str) -> rikka_net::HeaderMap {
        let mut headers = rikka_net::HeaderMap::new();
        let pairs = [
            (header::USER_AGENT.as_str(), USER_AGENT.to_owned()),
            ("x-super-properties", self.super_properties().to_owned()),
            ("x-discord-locale", self.locale()),
            ("x-discord-timezone", self.timezone.clone()),
            ("x-debug-options", "bugReporterEnabled".to_owned()),
            (header::ORIGIN.as_str(), ORIGIN.to_owned()),
            (header::REFERER.as_str(), referer.to_owned()),
            (header::ACCEPT_LANGUAGE.as_str(), self.locale()),
        ];
        for (name, value) in pairs {
            if let (Ok(name), Ok(value)) = (
                rikka_net::HeaderName::from_bytes(name.as_bytes()),
                rikka_net::HeaderValue::from_str(&value),
            ) {
                headers.insert(name, value);
            }
        }
        headers
    }

    pub(crate) async fn load(cache: &Path) -> Identity {
        let file = cache.join(BUILD_FILE);
        let cached = rikka_account_book::read_json::<CachedBuild>(&file)
            .ok()
            .flatten();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        if let Some(cached) = &cached
            && now.saturating_sub(cached.fetched) < BUILD_LASTS.as_secs()
        {
            return Identity::new(cached.number);
        }
        match fetch_build().await {
            Some(number) => {
                save_build(
                    &file,
                    &CachedBuild {
                        number,
                        fetched: now,
                    },
                );
                Identity::new(number)
            }
            None => Identity::new(cached.map_or(FALLBACK_BUILD, |cached| cached.number)),
        }
    }
}

fn save_build(file: &Path, build: &CachedBuild) {
    if let Err(error) = rikka_account_book::write_private_json(file, build) {
        tracing::debug!(%error, "failed to cache Discord build number");
    }
}

async fn fetch_build() -> Option<u32> {
    let client = rikka_net::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(10))
        .build()
        .ok()?;
    let page = client.get_text("https://discord.com/app").await.ok()?;
    let path = sentry_script(&page)?;
    let source = client.get_text(format!("{ORIGIN}{path}")).await.ok()?;
    build_number(&source)
}

fn sentry_script(page: &str) -> Option<&str> {
    page.match_indices(SENTRY_SCRIPT).find_map(|(at, _)| {
        let tail = &page[at..];
        let quoted = tail
            .find(|c: char| c == '"' || c == '\'' || c.is_whitespace())
            .unwrap_or(tail.len());
        let end = tail[..quoted].find(SCRIPT_EXTENSION)? + SCRIPT_EXTENSION.len();
        Some(&tail[..end])
    })
}

fn build_number(source: &str) -> Option<u32> {
    source.match_indices(BUILD_NUMBER).find_map(|(at, key)| {
        let rest = source[at + key.len()..]
            .strip_prefix(['"', '\''])
            .unwrap_or(&source[at + key.len()..]);
        let rest = rest.trim_start().strip_prefix([':', ','])?.trim_start();
        let rest = rest.strip_prefix(['"', '\'']).unwrap_or(rest);
        let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        let digits = &rest[..digits.min(BUILD_DIGITS_MOST)];
        if digits.len() < BUILD_DIGITS_LEAST {
            return None;
        }
        digits.parse().ok()
    })
}

fn system_locale() -> String {
    rikka_types::LocaleCategory::Messages
        .locale()
        .map_or_else(|| "en-US".to_owned(), |locale| locale.tag())
}

fn discord_locale(system: &str) -> String {
    let (language, region) = system.split_once('-').unwrap_or((system, ""));
    match (language, region) {
        ("en", "GB") => "en-GB".to_owned(),
        ("en", _) => "en-US".to_owned(),
        ("zh", "TW" | "HK") => "zh-TW".to_owned(),
        ("zh", _) => "zh-CN".to_owned(),
        ("pt", _) => "pt-BR".to_owned(),
        ("es", "ES") | ("es", "") => "es-ES".to_owned(),
        ("es", _) => "es-419".to_owned(),
        ("sv", _) => "sv-SE".to_owned(),
        ("nb" | "nn" | "no", _) => "no".to_owned(),
        (language, _) => language.to_owned(),
    }
}

fn random_uuid(mask: Option<[u8; 16]>) -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    if let Some(mask) = mask {
        for (byte, keep) in bytes.iter_mut().zip(mask) {
            *byte &= keep;
        }
    }
    let hex = rikka_types::hex::encode(bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}
