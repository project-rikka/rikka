use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use rikka_llm::wire::{self, client};
use rikka_llm::{Error, Result};
use rikka_net::{Bytes, Client, StatusCode, header};
use rikka_sync::Mutex;

use crate::document::Catalog;

const SOURCE: &str = "https://models.dev/api.json";
const FRESH_FOR: Duration = Duration::from_secs(60 * 60);
const RETRY_AFTER: Duration = Duration::from_secs(15 * 60);
const READ: Duration = Duration::from_secs(60);
const CONNECT: Duration = Duration::from_secs(15);
const LIMIT: usize = 64 * 1024 * 1024;
const FOLDER: &str = "llm";
const DOCUMENT: &str = "models.dev.json";
const TAG: &str = "models.dev.etag";

struct Held {
    catalog: Arc<Catalog>,
    tag: Option<String>,
    due: Instant,
}

enum Answer {
    Fresh {
        document: Bytes,
        tag: Option<String>,
    },
    Unchanged,
}

static HELD: Mutex<Option<Held>> = Mutex::new(None);
static ASKING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

impl Catalog {
    pub async fn current() -> Result<Arc<Catalog>> {
        let _turn = ASKING.lock().await;
        let (kept, tag) = match held() {
            Some(held) if held.due > Instant::now() => return Ok(held.catalog),
            Some(held) => (Some(held.catalog), held.tag),
            None => stored().await,
        };

        match ask(tag.as_deref().filter(|_| kept.is_some())).await {
            Ok(Answer::Fresh { document, tag }) => {
                let catalog = Arc::new(parsed(document.clone()).await?);
                store(document, tag.clone()).await;
                hold(&catalog, tag, FRESH_FOR);
                Ok(catalog)
            }
            Ok(Answer::Unchanged) => {
                let catalog = kept.ok_or_else(|| {
                    Error::Malformed("the model catalog was unchanged but none is kept".to_owned())
                })?;
                hold(&catalog, tag, FRESH_FOR);
                Ok(catalog)
            }
            Err(error) => match kept {
                Some(catalog) => {
                    tracing::warn!("failed to refresh the model catalog: {error}");
                    hold(&catalog, tag, RETRY_AFTER);
                    Ok(catalog)
                }
                None => Err(error),
            },
        }
    }
}

fn held() -> Option<Held> {
    HELD.lock().as_ref().map(|held| Held {
        catalog: Arc::clone(&held.catalog),
        tag: held.tag.clone(),
        due: held.due,
    })
}

fn hold(catalog: &Arc<Catalog>, tag: Option<String>, fresh_for: Duration) {
    *HELD.lock() = Some(Held {
        catalog: Arc::clone(catalog),
        tag,
        due: Instant::now() + fresh_for,
    });
}

async fn ask(tag: Option<&str>) -> Result<Answer> {
    let mut request = shared_client()?.get(SOURCE);
    if let Some(tag) = tag {
        request = request.header(header::IF_NONE_MATCH, tag);
    }
    let response = request.send().await.map_err(wire::translate)?;
    if response.status() == StatusCode::NOT_MODIFIED {
        return Ok(Answer::Unchanged);
    }
    let response = response.error_for_status().await.map_err(wire::translate)?;
    let tag = response
        .headers()
        .get(header::ETAG)
        .and_then(|tag| tag.to_str().ok())
        .map(str::to_owned);
    let document = response
        .bytes_limited(LIMIT)
        .await
        .map_err(wire::translate)?;
    Ok(Answer::Fresh { document, tag })
}

fn shared_client() -> Result<Client> {
    static SHARED: OnceLock<Client> = OnceLock::new();
    if let Some(shared) = SHARED.get() {
        return Ok(shared.clone());
    }
    let built = client(READ, CONNECT)?;
    Ok(SHARED.get_or_init(|| built).clone())
}

async fn parsed(document: Bytes) -> Result<Catalog> {
    tokio::task::spawn_blocking(move || Catalog::parse(&document))
        .await
        .map_err(|error| Error::Malformed(format!("failed to read the model catalog: {error}")))?
}

async fn stored() -> (Option<Arc<Catalog>>, Option<String>) {
    let read = tokio::task::spawn_blocking(|| {
        let folder = folder()?;
        let document = std::fs::read(folder.join(DOCUMENT)).ok()?;
        let catalog = Catalog::parse(&document)
            .inspect_err(|error| tracing::warn!("failed to read the kept model catalog: {error}"))
            .ok()?;
        let tag = std::fs::read_to_string(folder.join(TAG))
            .ok()
            .map(|tag| tag.trim().to_owned());
        Some((Arc::new(catalog), tag))
    })
    .await;
    match read {
        Ok(Some((catalog, tag))) => (Some(catalog), tag),
        Ok(None) => (None, None),
        Err(error) => {
            tracing::warn!("failed to read the kept model catalog: {error}");
            (None, None)
        }
    }
}

async fn store(document: Bytes, tag: Option<String>) {
    let stored = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        let Some(folder) = folder() else {
            return Ok(());
        };
        std::fs::create_dir_all(&folder)?;
        rikka_fs::replace(folder.join(DOCUMENT), &document)?;
        match tag {
            Some(tag) => rikka_fs::replace(folder.join(TAG), tag),
            None => match std::fs::remove_file(folder.join(TAG)) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
                _ => Ok(()),
            },
        }
    })
    .await;
    match stored {
        Ok(Ok(())) => {}
        Ok(Err(error)) => tracing::warn!("failed to keep the model catalog: {error}"),
        Err(error) => tracing::warn!("failed to keep the model catalog: {error}"),
    }
}

fn folder() -> Option<PathBuf> {
    rikka_dirs::Paths::resolve()
        .inspect_err(|error| tracing::warn!("failed to find the cache folder: {error}"))
        .ok()
        .map(|paths| paths.cache_dir().join(FOLDER))
}
