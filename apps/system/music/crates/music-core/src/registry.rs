use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_hifi::Hifi;

use crate::adapter::{Adapter, Setup};
use crate::cache::AudioCache;
use crate::deck::Deck;
use crate::error::{Error, Result};
use crate::provider::{Connection, Link, Prompt, Provider};
use crate::service::Service;

#[derive(Clone, Debug)]
pub struct Roots {
    pub cache: PathBuf,
    pub state: PathBuf,
}

#[derive(Clone)]
pub struct Providers {
    roots: Roots,
    audio: AudioCache,
    deck: Deck,
    registered: Vec<Registered>,
}

impl Providers {
    #[must_use]
    pub fn new(roots: Roots, audio: &AudioCache, output: &Hifi) -> Providers {
        Providers {
            roots,
            audio: audio.clone(),
            deck: Deck::new(output.clone()),
            registered: Vec::new(),
        }
    }

    #[must_use]
    pub fn adapter<A: Adapter>(self) -> Providers {
        let setup = self.setup_for(A::SERVICE);
        let adapter = A::open(&setup);
        self.register(A::SERVICE, adapter)
    }

    pub fn try_provider<P: Provider + 'static>(
        self,
        service: Service,
        open: impl FnOnce(&Setup) -> Result<P>,
    ) -> Result<Providers> {
        let setup = self.setup_for(service);
        let provider = open(&setup)?;
        Ok(self.register(service, provider))
    }

    fn setup_for(&self, service: Service) -> Setup {
        assert!(
            service.has_valid_slug(),
            "the music service slug {:?} must be lowercase letters and digits joined by single hyphens",
            service.slug
        );
        assert!(
            self.find(service.slug).is_none(),
            "the music service {:?} is registered twice",
            service.slug
        );
        Setup::new(
            self.roots.cache.join(service.slug),
            self.roots.state.join(service.slug),
            self.audio.for_provider(service.slug),
            self.deck.clone(),
        )
    }

    fn register<P: Provider + 'static>(mut self, service: Service, provider: P) -> Providers {
        self.registered.push(Registered {
            service,
            provider: Arc::new(provider),
        });
        self
    }

    pub fn services(&self) -> impl Iterator<Item = Service> + '_ {
        self.registered.iter().map(|registered| registered.service)
    }

    #[must_use]
    pub fn find(&self, slug: &str) -> Option<&Registered> {
        self.registered
            .iter()
            .find(|registered| registered.service.slug == slug)
    }

    #[must_use]
    pub fn output(&self) -> &Hifi {
        self.deck.hifi()
    }
}

impl std::fmt::Debug for Providers {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_list()
            .entries(self.services().map(|service| service.slug))
            .finish()
    }
}

#[derive(Clone)]
pub struct Registered {
    service: Service,
    provider: Arc<dyn Provider>,
}

impl Registered {
    #[must_use]
    pub fn service(&self) -> Service {
        self.service
    }

    pub async fn resume(&self, folder: &Path) -> Result<Option<Connection>> {
        match self.provider.resume(folder).await {
            Ok(link) => Ok(link.map(|link| self.connection(link))),
            Err(Error::Refused(reason)) => {
                tracing::warn!("saved {} sign-in expired: {reason}", self.service.name);
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    pub async fn sign_in(&self, folder: &Path, prompt: Prompt) -> Result<Connection> {
        let link = self.provider.sign_in(folder, prompt).await?;
        Ok(self.connection(link))
    }

    fn connection(&self, link: Link) -> Connection {
        Connection {
            service: self.service,
            account: link.account,
            catalog: link.catalog,
            playback: link.playback,
        }
    }
}

impl std::fmt::Debug for Registered {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Registered")
            .field("service", &self.service)
            .finish_non_exhaustive()
    }
}
