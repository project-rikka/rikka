use std::path::{Path, PathBuf};
use std::sync::Arc;

use music_core::{
    Adapter, Deck, Error, Kind, Link, Mark, Prompt, Provider, ProviderCache, Result, Service,
    Setup, async_trait,
};
use music_qobuz_keys::Vault;
use music_stream::Streamer;
use rikka_net::Client;
use rikka_symbols::MaterialSymbol;
use serde_json::Value;

use crate::api::{self, Api};
use crate::bundle::App;
use crate::catalog::QobuzCatalog;
use crate::origin::QobuzOrigin;
use crate::{oauth, session};

#[derive(Debug)]
pub struct Qobuz {
    cache: PathBuf,
    vault: PathBuf,
    audio: ProviderCache,
    deck: Deck,
}

impl Qobuz {
    async fn link(&self, http: Client, app: App, token: String) -> Result<Link> {
        let (api, login) = match Api::open(http.clone(), app, token.clone()).await {
            Ok(api) => api,
            Err(Error::Unreadable(stale)) => {
                tracing::info!("cached Qobuz bundle is stale, refetching: {stale}");
                let fresh = App::current(&http, &self.cache, true).await?;
                Api::open(http, fresh, token).await?
            }
            Err(error) => return Err(error),
        };
        self.linked(Arc::new(api), &login).await
    }

    async fn linked(&self, api: Arc<Api>, login: &Value) -> Result<Link> {
        let member = session::member(login)?;
        let vault = Arc::new(Vault::open(&self.vault).await?);
        Ok(Link {
            account: member.account,
            catalog: Arc::new(QobuzCatalog::new(Arc::clone(&api), member.id)),
            playback: Arc::new(Streamer::start(
                Arc::new(QobuzOrigin::new(api, vault)),
                self.audio.clone(),
                self.deck.clone(),
            )?),
        })
    }
}

impl Adapter for Qobuz {
    const SERVICE: Service = Service {
        slug: "qobuz",
        name: "Qobuz",
        mark: Mark::Symbol(MaterialSymbol::HIGH_RES.codepoint()),
        kind: Kind::Streaming,
    };

    fn open(setup: &Setup) -> Qobuz {
        Qobuz {
            cache: setup.cache().to_owned(),
            vault: setup.state().to_owned(),
            audio: setup.audio().clone(),
            deck: setup.deck().clone(),
        }
    }
}

#[async_trait]
impl Provider for Qobuz {
    async fn resume(&self, folder: &Path) -> Result<Option<Link>> {
        let Some(token) = session::load_token(folder).await? else {
            return Ok(None);
        };
        let http = api::http();
        let app = App::current(&http, &self.cache, false).await?;
        self.link(http, app, token).await.map(Some)
    }

    async fn sign_in(&self, folder: &Path, prompt: Prompt) -> Result<Link> {
        let http = api::http();
        let app = App::current(&http, &self.cache, false).await?;
        let code = oauth::authorize(&app, prompt).await?;
        let token = api::exchange(&http, &app, &code).await?;
        let link = self.link(http, app, token.clone()).await?;
        session::save_token(folder, &token).await?;
        Ok(link)
    }
}
