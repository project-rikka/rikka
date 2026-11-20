use std::path::Path;
use std::sync::Arc;

use music_core::{
    Account, Adapter, Deck, Error, Kind, Link, Mark, Prompt, Provider, ProviderCache, Result,
    Service, Setup, SignInStep, WebSignIn, async_trait,
};
use music_stream::Streamer;
use rikka_symbols::BrandSymbol;
use serde_json::Value;

use crate::api::Api;
use crate::catalog::SoundCloudCatalog;
use crate::client_id::ClientIds;
use crate::origin::SoundCloudOrigin;
use crate::parse::{self, text};
use crate::session::{PROOF, Session};

const SIGN_IN: &str = "https://soundcloud.com/signin";
const LANDING: &str = "soundcloud.com";
const COOKIES_OF: &str = "https://soundcloud.com/";

#[derive(Debug)]
pub struct SoundCloud {
    ids: Arc<ClientIds>,
    audio: ProviderCache,
    deck: Deck,
}

impl SoundCloud {
    async fn link(&self, session: Session) -> Result<Link> {
        let api = Arc::new(Api::new(Arc::clone(&self.ids), session));
        let account = account(&api.get("me", &[]).await?)?;
        let origin = SoundCloudOrigin::new(Arc::clone(&api));
        Ok(Link {
            catalog: Arc::new(SoundCloudCatalog::new(api, account.id.clone())),
            playback: Arc::new(Streamer::start(
                Arc::new(origin),
                self.audio.clone(),
                self.deck.clone(),
            )?),
            account,
        })
    }
}

impl Adapter for SoundCloud {
    const SERVICE: Service = Service {
        slug: "soundcloud",
        name: "SoundCloud",
        mark: Mark::Brand(BrandSymbol::SOUNDCLOUD.codepoint()),
        kind: Kind::Streaming,
    };

    fn open(setup: &Setup) -> SoundCloud {
        SoundCloud {
            ids: Arc::new(ClientIds::new(setup.cache().to_owned())),
            audio: setup.audio().clone(),
            deck: setup.deck().clone(),
        }
    }
}

#[async_trait]
impl Provider for SoundCloud {
    async fn resume(&self, folder: &Path) -> Result<Option<Link>> {
        let Some(session) = Session::open(folder).await? else {
            return Ok(None);
        };
        self.link(session).await.map(Some)
    }

    async fn sign_in(&self, folder: &Path, prompt: Prompt) -> Result<Link> {
        let (request, answer) = WebSignIn::new(SIGN_IN, LANDING, COOKIES_OF, PROOF);
        prompt(SignInStep::Web(request));
        let cookies = answer.await?;
        self.link(Session::fill(folder, &cookies).await?).await
    }
}

fn account(me: &Value) -> Result<Account> {
    let id = me
        .get("id")
        .and_then(parse::id)
        .ok_or_else(|| Error::Refused("SoundCloud reports no signed-in account".into()))?;
    Ok(Account {
        name: text(me, "username")
            .or_else(|| text(me, "permalink"))
            .unwrap_or(id.as_str())
            .to_owned(),
        id: id.to_string(),
    })
}
