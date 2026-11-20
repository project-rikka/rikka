use std::path::Path;
use std::sync::Arc;

use music_core::{
    Account, Adapter, Deck, Error, Kind, Link, Mark, Prompt, Provider, ProviderCache, Result,
    Service, Setup, SignInStep, WebSignIn, async_trait,
};
use music_stream::Streamer;
use rikka_symbols::BrandSymbol;
use serde_json::json;

use crate::catalog::YouTubeCatalog;
use crate::clients::Client;
use crate::innertube::Innertube;
use crate::jar::{Jar, PROOF};
use crate::json::Walk;
use crate::playback::YouTubeOrigin;
use crate::solver::Solving;

const SIGN_IN: &str = "https://accounts.google.com/ServiceLogin?ltmpl=music&service=youtube&passive=true&continue=https%3A%2F%2Fwww.youtube.com%2Fsignin%3Faction_handle_signin%3Dtrue%26next%3Dhttps%253A%252F%252Fmusic.youtube.com%252F";
const LANDING: &str = "music.youtube.com";
const COOKIES_OF: &str = "https://music.youtube.com/";
const DEFAULT_REGION: &str = "US";

#[derive(Debug)]
pub struct YouTubeMusic {
    solving: Arc<Solving>,
    audio: ProviderCache,
    deck: Deck,
    region: String,
}

impl YouTubeMusic {
    async fn link(&self, jar: Jar) -> Result<Link> {
        let tube = Arc::new(Innertube::new(jar, self.region.clone()));
        let account = account(&tube).await?;
        let origin = YouTubeOrigin::new(Arc::clone(&tube), Arc::clone(&self.solving));
        Ok(Link {
            catalog: Arc::new(YouTubeCatalog::new(tube, account.name.clone())),
            playback: Arc::new(Streamer::start(
                Arc::new(origin),
                self.audio.clone(),
                self.deck.clone(),
            )?),
            account,
        })
    }
}

impl Adapter for YouTubeMusic {
    const SERVICE: Service = Service {
        slug: "youtube-music",
        name: "YouTube Music",
        mark: Mark::Brand(BrandSymbol::YOUTUBE.codepoint()),
        kind: Kind::Streaming,
    };

    fn open(setup: &Setup) -> YouTubeMusic {
        YouTubeMusic {
            solving: Arc::new(Solving::new(setup.cache().to_owned())),
            audio: setup.audio().clone(),
            deck: setup.deck().clone(),
            region: region(),
        }
    }
}

#[async_trait]
impl Provider for YouTubeMusic {
    async fn resume(&self, folder: &Path) -> Result<Option<Link>> {
        let Some(jar) = Jar::open(folder).await? else {
            return Ok(None);
        };
        self.link(jar).await.map(Some)
    }

    async fn sign_in(&self, folder: &Path, prompt: Prompt) -> Result<Link> {
        let (request, answer) = WebSignIn::new(SIGN_IN, LANDING, COOKIES_OF, PROOF);
        prompt(SignInStep::Web(request));
        let cookies = answer.await?;
        self.link(Jar::fill(folder, &cookies).await?).await
    }
}

async fn account(tube: &Innertube) -> Result<Account> {
    let answer = tube
        .call("account/account_menu", Client::Music, json!({}))
        .await?;
    let header = answer
        .first("activeAccountHeaderRenderer")
        .ok_or_else(|| Error::Refused("YouTube Music reports no signed-in account".into()))?;
    let name = header.message(&["accountName"]);
    let handle = header
        .message(&["email"])
        .or_else(|| header.message(&["channelHandle"]));
    let id = handle
        .clone()
        .or_else(|| name.clone())
        .ok_or_else(|| Error::Unreadable("the signed-in account has no name".into()))?;
    Ok(Account {
        name: name.or(handle).unwrap_or_else(|| id.clone()),
        id,
    })
}

fn region() -> String {
    rikka_types::LocaleCategory::Messages
        .locale()
        .and_then(|locale| locale.territory().map(str::to_ascii_uppercase))
        .unwrap_or_else(|| DEFAULT_REGION.to_owned())
}
