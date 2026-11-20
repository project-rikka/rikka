use std::path::{Path, PathBuf};
use std::sync::Arc;

use discord_core::{Account, Updates, UserId, account_secret};
use discord_wire as wire;
use discord_wire::snowflake;
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;
use tokio::sync::OnceCell;

use crate::client::Client;
use crate::error::{Error, Result};
use crate::gateway::First;
use crate::identity::Identity;
use crate::rest::{Rest, Token};
use crate::runtime;
use crate::session::Session;
use crate::signin;
use crate::signing::{Method, Prompt};

const SIGN_IN_LABEL: &str = "Discord sign-in";
const ACCOUNT_AVATAR: u32 = 128;

pub struct Discord {
    shared: Arc<Shared>,
}

pub struct Link {
    pub account: Account,
    pub client: Client,
    pub updates: Updates,
}

struct Shared {
    cache: PathBuf,
    identity: OnceCell<Arc<Identity>>,
}

#[derive(Serialize, Deserialize)]
struct SavedSession {
    token: String,
}

impl Discord {
    #[must_use]
    pub fn open(cache: impl Into<PathBuf>) -> Discord {
        Discord {
            shared: Arc::new(Shared {
                cache: cache.into(),
                identity: OnceCell::new(),
            }),
        }
    }

    pub async fn resume(&self, folder: &Path) -> discord_core::Result<Option<Link>> {
        let Some(saved) = account_secret(folder).kept::<SavedSession>().await? else {
            return Ok(None);
        };
        let shared = Arc::clone(&self.shared);
        let handle = runtime::handle();
        let work = connect(shared, handle.clone(), saved.token);
        match runtime::run(&handle, work).await {
            Ok(link) => Ok(Some(link)),
            Err(Error::SignedOut) => {
                tracing::warn!("saved Discord sign-in expired");
                Ok(None)
            }
            Err(error_message) => Err(error_message.into()),
        }
    }

    pub async fn sign_in(
        &self,
        folder: &Path,
        method: Method,
        prompt: Prompt,
    ) -> discord_core::Result<Link> {
        let shared = Arc::clone(&self.shared);
        let handle = runtime::handle();
        let folder = folder.to_owned();
        let runtime = handle.clone();
        let work = async move {
            let identity = shared.identity().await;
            let token = match method {
                Method::QrCode => signin::qr(prompt, identity).await?,
                Method::Browser => signin::browser(prompt).await?,
            };
            account_secret(&folder)
                .keep(
                    SIGN_IN_LABEL,
                    &SavedSession {
                        token: token.clone(),
                    },
                )
                .await
                .map_err(discord_core::Error::from)?;
            connect(shared, runtime, token).await
        };
        Ok(runtime::run(&handle, work).await?)
    }
}

impl std::fmt::Debug for Discord {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Discord")
            .field("cache", &self.shared.cache)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Link {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Link")
            .field("account", &self.account)
            .finish_non_exhaustive()
    }
}

impl Shared {
    async fn identity(&self) -> Arc<Identity> {
        Arc::clone(
            self.identity
                .get_or_init(|| async { Arc::new(Identity::load(&self.cache).await) })
                .await,
        )
    }
}

async fn connect(shared: Arc<Shared>, runtime: Handle, token: String) -> Result<Link> {
    let base = shared.identity().await;
    let identity = Arc::new(Identity::new(base.build()));
    let rest = Rest::new(Arc::new(Token::new(token)), identity)?;
    let me: wire::User = rest.get("/users/@me").parse().await?;
    let (session, updates, first) = Session::start(runtime, rest);
    match first.await {
        Ok(First::SignedOut) => {
            session.close();
            return Err(Error::SignedOut);
        }
        Ok(First::Ready | First::Offline) => {}
        Err(_) => return Err(Error::Cancelled),
    }
    let name = me
        .global_name
        .clone()
        .filter(|name| !name.is_empty())
        .or_else(|| me.username.clone())
        .unwrap_or_default();
    let avatar =
        discord_translate::avatar(&me.id, me.avatar.as_deref(), me.discriminator.as_deref());
    Ok(Link {
        account: Account {
            id: snowflake::id::<UserId>(&me.id),
            name,
            avatar: Some(avatar.url_for_size(ACCOUNT_AVATAR)),
        },
        client: Client::new(session),
        updates,
    })
}
