use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rikka_llm::wire::{self, client};
use rikka_llm::{Error, Result};
use rikka_net::{Client, header};
use rikka_sync::Mutex;

const MANIFEST: &str = "https://antigravity-hub-auto-updater-974169037036.us-central1.run.app/manifest/latest-arm64-mac.yml";
const MANIFEST_AGENT: &str = "electron-builder";
const MANIFEST_LIMIT: usize = 4096;
const MANIFEST_READ: Duration = Duration::from_secs(10);
const MANIFEST_CONNECT: Duration = Duration::from_secs(10);
const FRESH_FOR: Duration = Duration::from_secs(6 * 60 * 60);
const RETRY_AFTER: Duration = Duration::from_secs(15 * 60);

const FALLBACK: &str = "2.29.0";
const CHANGELISTS: [(&str, &str); 3] = [
    ("2.23.0", "998057262"),
    ("2.24.0", "1000335682"),
    ("2.29.0", "1013060670"),
];
const PLATFORM: &str = "os_type=darwin; arch=arm64";
const NODE_CLIENT: &str = "google-api-nodejs-client/10.3.0";

struct Known {
    version: String,
    due: Instant,
}

static KNOWN: Mutex<Option<Known>> = Mutex::new(None);
static REFRESHING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static SCHEDULED: AtomicBool = AtomicBool::new(false);

pub(crate) fn version() -> String {
    let (version, due) = match &*KNOWN.lock() {
        Some(known) => (known.version.clone(), known.due <= Instant::now()),
        None => (FALLBACK.to_owned(), true),
    };
    if due
        && let Ok(runtime) = tokio::runtime::Handle::try_current()
        && !SCHEDULED.swap(true, Ordering::AcqRel)
    {
        runtime.spawn(async {
            refresh().await;
            SCHEDULED.store(false, Ordering::Release);
        });
    }
    version
}

pub(crate) fn agent() -> String {
    let version = version();
    let changelist = CHANGELISTS
        .iter()
        .find(|(release, _)| *release == version)
        .map(|(_, changelist)| format!("; cl={changelist}"))
        .unwrap_or_default();
    format!("antigravity/hub/{version} (aidev_client; {PLATFORM}{changelist})")
}

pub(crate) fn onboarding_agent() -> String {
    format!("{} {NODE_CLIENT}", agent())
}

pub(crate) async fn refresh() {
    let _turn = REFRESHING.lock().await;
    let due = KNOWN
        .lock()
        .as_ref()
        .is_none_or(|known| known.due <= Instant::now());
    if !due {
        return;
    }
    let latest = latest().await;
    let mut known = KNOWN.lock();
    *known = Some(match latest {
        Ok(version) => Known {
            version,
            due: Instant::now() + FRESH_FOR,
        },
        Err(error) => {
            tracing::warn!("failed to read the latest Antigravity release: {error}");
            Known {
                version: known
                    .take()
                    .map_or_else(|| FALLBACK.to_owned(), |kept| kept.version),
                due: Instant::now() + RETRY_AFTER,
            }
        }
    });
}

async fn latest() -> Result<String> {
    let manifest = manifest_client()?
        .get(MANIFEST)
        .header(header::USER_AGENT, MANIFEST_AGENT)
        .header(header::CACHE_CONTROL, "no-cache")
        .send()
        .await
        .map_err(wire::translate)?
        .error_for_status()
        .await
        .map_err(wire::translate)?
        .bytes_limited(MANIFEST_LIMIT)
        .await
        .map_err(wire::translate)?;
    released(&String::from_utf8_lossy(&manifest))
        .ok_or_else(|| Error::Malformed("the release manifest names no version".to_owned()))
}

fn manifest_client() -> Result<Client> {
    static SHARED: OnceLock<Client> = OnceLock::new();
    if let Some(shared) = SHARED.get() {
        return Ok(shared.clone());
    }
    let built = client(MANIFEST_READ, MANIFEST_CONNECT)?;
    Ok(SHARED.get_or_init(|| built).clone())
}

fn released(manifest: &str) -> Option<String> {
    manifest
        .lines()
        .find_map(|line| line.strip_prefix("version:"))
        .map(|version| version.trim().trim_matches(['"', '\'']))
        .filter(|version| is_release(version))
        .map(str::to_owned)
}

fn is_release(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}
