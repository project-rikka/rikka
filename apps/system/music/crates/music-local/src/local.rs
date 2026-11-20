use std::path::Path;
use std::sync::Arc;

use music_core::{
    Account, Catalog, Kind, Link, Mark, Playback, Prompt, Provider, Result, Roots, Service, Setup,
    async_trait,
};
use music_library::Library;
use music_stream::Streamer;
use rikka_symbols::MaterialSymbol;

use crate::catalog::LocalCatalog;
use crate::origin::LocalOrigin;
use crate::plays;

const DATABASE: &str = "library.sqlite";
const ARTWORK: &str = "artwork";

pub struct Local {
    catalog: Arc<LocalCatalog>,
    playback: Arc<Streamer>,
}

impl Local {
    pub const SERVICE: Service = Service {
        slug: "local",
        name: "Local music",
        mark: Mark::Symbol(MaterialSymbol::LIBRARY_MUSIC.codepoint()),
        kind: Kind::Local,
    };

    pub fn library(roots: &Roots) -> music_library::Setup {
        music_library::Setup::new(
            roots.state.join(Local::SERVICE.slug).join(DATABASE),
            roots.cache.join(Local::SERVICE.slug).join(ARTWORK),
        )
    }

    pub fn new(setup: &Setup, library: Library) -> Result<Local> {
        let origin = LocalOrigin::new(library.clone());
        let playback = Arc::new(Streamer::start(
            Arc::new(origin),
            setup.audio().clone(),
            setup.deck().clone(),
        )?);
        plays::count(playback.events(), library.clone())?;
        Ok(Local {
            catalog: Arc::new(LocalCatalog::new(library)),
            playback,
        })
    }

    fn link(&self) -> Link {
        Link {
            account: Account {
                id: Local::SERVICE.slug.to_owned(),
                name: Local::SERVICE.name.to_owned(),
            },
            catalog: Arc::clone(&self.catalog) as Arc<dyn Catalog>,
            playback: Arc::clone(&self.playback) as Arc<dyn Playback>,
        }
    }
}

impl std::fmt::Debug for Local {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Local").finish_non_exhaustive()
    }
}

#[async_trait]
impl Provider for Local {
    async fn resume(&self, _folder: &Path) -> Result<Option<Link>> {
        Ok(Some(self.link()))
    }

    async fn sign_in(&self, _folder: &Path, _prompt: Prompt) -> Result<Link> {
        Ok(self.link())
    }
}
