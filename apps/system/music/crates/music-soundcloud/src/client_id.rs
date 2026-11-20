use std::fs;
use std::path::PathBuf;

use music_core::{Error, Result};
use music_stream::{Scan, answer_text};
use rikka_net::Client;
use tokio::sync::Mutex;

const HOME: &str = "https://soundcloud.com/";
const FILE: &str = "client-id";
const ASSETS: &str = "https://a-v2.sndcdn.com/assets/";
const CLIENT_ID_LEN: usize = 32;

#[derive(Debug)]
pub(crate) struct ClientIds {
    file: PathBuf,
    cached: Mutex<Option<String>>,
}

impl ClientIds {
    pub(crate) fn new(cache: PathBuf) -> ClientIds {
        ClientIds {
            file: cache.join(FILE),
            cached: Mutex::new(None),
        }
    }

    pub(crate) async fn current(&self, http: &Client) -> Result<String> {
        let mut cached = self.cached.lock().await;
        if let Some(known) = cached.as_ref() {
            return Ok(known.clone());
        }
        let known = match self.load_cached() {
            Some(cached) => cached,
            None => self.discover(http).await?,
        };
        *cached = Some(known.clone());
        Ok(known)
    }

    pub(crate) async fn renew(&self, http: &Client, stale: &str) -> Result<String> {
        let mut cached = self.cached.lock().await;
        if let Some(fresh) = cached.as_ref().filter(|known| *known != stale) {
            return Ok(fresh.clone());
        }
        let fresh = self.discover(http).await?;
        *cached = Some(fresh.clone());
        Ok(fresh)
    }

    fn load_cached(&self) -> Option<String> {
        fs::read_to_string(&self.file)
            .ok()
            .map(|text| text.trim().to_owned())
            .filter(|id| id.len() == 32 && id.bytes().all(|byte| byte.is_ascii_alphanumeric()))
    }

    async fn discover(&self, http: &Client) -> Result<String> {
        let page = answer_text(http.get(HOME)).await?;
        let client_id = match hydrated(&page) {
            Some(client_id) => client_id,
            None => scripted(http, &page).await?,
        };
        let stored = self
            .file
            .parent()
            .map_or(Ok(()), fs::create_dir_all)
            .and_then(|()| fs::write(&self.file, &client_id));
        if let Err(error) = stored {
            tracing::warn!("failed to cache SoundCloud client id: {error}");
        }
        Ok(client_id)
    }
}

async fn scripted(http: &Client, page: &str) -> Result<String> {
    for script in scripts(page).into_iter().rev() {
        match answer_text(http.get(script.as_str())).await {
            Ok(source) => {
                if let Some(client_id) = assigned(&source) {
                    return Ok(client_id);
                }
            }
            Err(error) => tracing::debug!("failed to read {script}: {error}"),
        }
    }
    Err(Error::Unreadable("soundcloud.com has no client id".into()))
}

fn scripts(page: &str) -> Vec<String> {
    Scan::new(page)
        .matches("<script", |mut scan| {
            let tag = scan.take(1..=usize::MAX, |c| c != '>')?;
            let mut inside = Scan::new(tag);
            if !inside.find(&format!(r#"src="{ASSETS}"#)) {
                return None;
            }
            let path = inside.take(1..=usize::MAX, |c| c != '"')?;
            inside.expect("\"")?;
            path.ends_with(".js").then(|| format!("{ASSETS}{path}"))
        })
        .collect()
}

fn hydrated(page: &str) -> Option<String> {
    Scan::new(page)
        .matches(r#""hydratable":"apiClient","data":{"id":""#, client_id)
        .next()
}

fn assigned(source: &str) -> Option<String> {
    Scan::new(source)
        .matches("client_id", |mut scan| {
            scan.skip_while(char::is_whitespace);
            scan.expect(":")?;
            scan.skip_while(char::is_whitespace);
            scan.expect("\"")?;
            client_id(scan)
        })
        .next()
}

fn client_id(mut scan: Scan<'_>) -> Option<String> {
    let id = scan.take(CLIENT_ID_LEN..=CLIENT_ID_LEN, |c| c.is_ascii_alphanumeric())?;
    scan.expect("\"")?;
    Some(id.to_owned())
}
