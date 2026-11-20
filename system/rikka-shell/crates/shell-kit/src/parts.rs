use std::sync::mpsc::Receiver;

use rikka_seam::{Call, Seam};

use crate::KeyboardState;
use crate::builder::Watcher;
use crate::domains::Domains;
use crate::erased::ErasedSurface;
use crate::host::Host;
use crate::idle::Idles;
use crate::session::Session;
use crate::shared_theme::SharedTheme;
use crate::waker::LoopWaker;
use crate::wall::WallClock;

pub struct Parts {
    pub name: String,
    pub shared_theme: SharedTheme,
    pub surfaces: Vec<ErasedSurface>,
    pub watchers: Vec<Watcher>,
    pub seam: Seam<Host>,
    pub waker: LoopWaker,
    pub calls: Receiver<Call>,
    pub session: Option<Session>,
    pub domains: Domains,
    pub wall: WallClock,
    pub idles: Idles,
    pub keyboard: KeyboardState,
    pub host: Host,
}

impl std::fmt::Debug for Parts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Parts")
            .field("name", &self.name)
            .field("surfaces", &self.surfaces)
            .field("watching", &self.watchers.len())
            .field("seam", &self.seam)
            .field("session", &self.session)
            .field("domains", &self.domains)
            .field("host", &self.host)
            .finish_non_exhaustive()
    }
}
