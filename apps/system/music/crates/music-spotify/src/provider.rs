use std::path::Path;
use std::sync::Arc;

use music_core::{
    Account, Adapter, Deck, Kind, Link, Mark, Prompt, Provider, ProviderCache, Result, Service,
    Setup, async_trait,
};
use music_spotify_api::SpClient;
use music_spotify_session::Session;
use music_stream::Streamer;
use rikka_symbols::BrandSymbol;

use crate::auth::Keys;
use crate::catalog::SpotifyCatalog;
use crate::origin::SpotifyOrigin;
use crate::worker;

#[derive(Debug)]
pub struct Spotify {
    audio: ProviderCache,
    deck: Deck,
}

impl Adapter for Spotify {
    const SERVICE: Service = Service {
        slug: "spotify",
        name: "Spotify",
        mark: Mark::Brand(BrandSymbol::SPOTIFY.codepoint()),
        kind: Kind::Streaming,
    };

    fn open(setup: &Setup) -> Spotify {
        Spotify {
            audio: setup.audio().clone(),
            deck: setup.deck().clone(),
        }
    }
}

#[async_trait]
impl Provider for Spotify {
    async fn resume(&self, folder: &Path) -> Result<Option<Link>> {
        let keys = Keys::new(folder.to_owned());
        let (audio, deck) = (self.audio.clone(), self.deck.clone());
        worker::run(async move {
            keys.resume()
                .await?
                .map(|session| linked(session, audio, deck))
                .transpose()
        })
        .await
    }

    async fn sign_in(&self, folder: &Path, prompt: Prompt) -> Result<Link> {
        let keys = Keys::new(folder.to_owned());
        let (audio, deck) = (self.audio.clone(), self.deck.clone());
        worker::run(async move { linked(keys.sign_in(prompt).await?, audio, deck) }).await
    }
}

fn linked(session: Session, audio: ProviderCache, deck: Deck) -> Result<Link> {
    let id = session.account().username;
    let api = SpClient::new(session.device().clone(), session.credentials());
    let origin = Arc::new(SpotifyOrigin::new(session.clone(), api.clone()));
    Ok(Link {
        account: Account {
            name: id.clone(),
            id,
        },
        catalog: Arc::new(SpotifyCatalog::new(session, api)),
        playback: Arc::new(Streamer::start(origin, audio, deck)?),
    })
}
