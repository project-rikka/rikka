use std::path::{Path, PathBuf};

use crate::cache::ProviderCache;
use crate::deck::Deck;
use crate::provider::Provider;
use crate::service::Service;

pub trait Adapter: Provider + Sized + 'static {
    const SERVICE: Service;

    fn open(setup: &Setup) -> Self;
}

#[derive(Clone, Debug)]
pub struct Setup {
    cache: PathBuf,
    state: PathBuf,
    audio: ProviderCache,
    deck: Deck,
}

impl Setup {
    pub(crate) fn new(cache: PathBuf, state: PathBuf, audio: ProviderCache, deck: Deck) -> Setup {
        Setup {
            cache,
            state,
            audio,
            deck,
        }
    }

    #[must_use]
    pub fn cache(&self) -> &Path {
        &self.cache
    }

    #[must_use]
    pub fn state(&self) -> &Path {
        &self.state
    }

    #[must_use]
    pub fn audio(&self) -> &ProviderCache {
        &self.audio
    }

    #[must_use]
    pub fn deck(&self) -> &Deck {
        &self.deck
    }
}
