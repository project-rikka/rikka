use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use rikka_clipboard::Clipboard;
use rikka_nested::Nested;
use rikka_picture::SharedGpu;
use rikka_sync::Mutex;
use rikka_web_crx::Identity;

use crate::channel::{self, Link};
use crate::error::{Error, Result};
use crate::outbox::Outbox;
use crate::process::launch::{self, Launch};
use crate::process::profile::Profile;
use crate::process::{locate, manifest, supervise};
use crate::runtime::Runtime;
use crate::runtime::core::{Core, Parting};
use crate::runtime::dispatch::Inlet;
use crate::runtime::display;
use crate::runtime::downloads::Poller;
use crate::runtime::state::State;

const EXTENSION_PATIENCE: Duration = Duration::from_secs(20);

#[must_use]
#[derive(Debug, Default)]
pub struct RuntimeBuilder {
    profile: Profile,
    gpu: Option<SharedGpu>,
    clipboard: Option<Clipboard>,
}

impl RuntimeBuilder {
    pub fn profile(mut self, profile: Profile) -> RuntimeBuilder {
        self.profile = profile;
        self
    }

    pub fn gpu(mut self, gpu: SharedGpu) -> RuntimeBuilder {
        self.gpu = Some(gpu);
        self
    }

    pub fn clipboard(mut self, clipboard: Clipboard) -> RuntimeBuilder {
        self.clipboard = Some(clipboard);
        self
    }

    pub fn start(self) -> Result<Runtime> {
        let program = locate::chrome()?;
        let host = locate::host()?;
        let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .ok_or_else(|| {
                Error::Channel(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "XDG_RUNTIME_DIR is unset",
                ))
            })?;
        let folder = self.profile.prepare()?;
        let extension_id = Identity::rikka().extension_id();
        manifest::install(folder.path(), &host, &extension_id)?;
        let mut nested = Nested::builder();
        if let Some(gpu) = self.gpu {
            nested = nested.gpu(gpu);
        }
        if let Some(clipboard) = self.clipboard {
            nested = nested.clipboard(clipboard);
        }
        let nested = nested
            .start()
            .map_err(|error| Error::Display(error.to_string()))?;
        let (rendezvous, listener) = channel::open(&runtime_dir).map_err(Error::Channel)?;
        let (link, writer) = Link::start().map_err(Error::Channel)?;
        let (downloads, kicks) = Poller::new();
        let core = Arc::new_cyclic(|this| Core {
            link: Arc::new(link),
            nested: nested.clone(),
            state: Mutex::new(State::default()),
            outbox: Outbox::default(),
            claim_page: format!("chrome-extension://{extension_id}/claim.html"),
            extension_id,
            this: this.clone(),
            downloads,
            parting: Mutex::new(None),
        });
        kicks
            .follow(Arc::downgrade(&core))
            .map_err(Error::Channel)?;
        let display = match nested.events() {
            Some(events) => {
                Some(display::follow(events, Arc::downgrade(&core)).map_err(Error::Channel)?)
            }
            None => None,
        };
        let listening = channel::serve(
            listener,
            EXTENSION_PATIENCE,
            Arc::new(Inlet(Arc::downgrade(&core))),
        )
        .map_err(Error::Channel)?;
        let child = launch::launch(&Launch {
            program: &program,
            profile: folder.path(),
            display: nested.socket(),
            channel: &rendezvous.path(),
        })?;
        let exited = Arc::downgrade(&core);
        let watch = supervise::watch(child, folder, move |exit| {
            if let Some(core) = exited.upgrade() {
                core.exited(exit);
            }
        })
        .map_err(Error::Spawn)?;
        *core.parting.lock() = Some(Parting {
            watch,
            listening,
            rendezvous,
            nested,
            display,
            writer,
        });
        Ok(Runtime::from_core(core))
    }
}
