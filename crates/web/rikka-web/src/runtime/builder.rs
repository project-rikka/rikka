use std::sync::Arc;

use rikka_clipboard::Clipboard;
use rikka_gecko_protocol::Target;
use rikka_nested::Nested;
use rikka_picture::SharedGpu;

use crate::error::{Error, Result};
use crate::preferences::{Locales, Preferences};
use crate::process::launch::launch;
use crate::process::locate;
use crate::process::profile::Profile;
use crate::process::supervise;
use crate::runtime::core::RuntimeCore;
use crate::runtime::dispatch::LISTENED;
use crate::runtime::handle::Runtime;
use crate::wire::{Link, reader, writer};

#[must_use]
#[derive(Debug, Default)]
pub struct RuntimeBuilder {
    profile: Profile,
    locales: Locales,
    preferences: Preferences,
    gpu: Option<SharedGpu>,
    clipboard: Option<Clipboard>,
}

impl RuntimeBuilder {
    pub fn profile(mut self, profile: Profile) -> RuntimeBuilder {
        self.profile = profile;
        self
    }

    pub fn locales(mut self, locales: Locales) -> RuntimeBuilder {
        self.locales = locales;
        self
    }

    pub fn preferences(mut self, preferences: Preferences) -> RuntimeBuilder {
        self.preferences = preferences;
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
        let program = locate::program()?;
        let ephemeral = self.profile.is_ephemeral();
        let folder = self.profile.prepare()?;
        let nested = self.display()?;
        let launched = launch(&program, folder.path(), nested.socket())?;
        let reading = launched.channel.try_clone().map_err(Error::Spawn)?;
        let writing = launched.channel.try_clone().map_err(Error::Spawn)?;
        let (outgoing, writer) = writer::spawn(writing).map_err(Error::Spawn)?;
        let link = Arc::new(Link::new(launched.channel, outgoing));
        let core = Arc::new(RuntimeCore::new(
            Arc::clone(&link),
            nested.clone(),
            ephemeral,
        ));
        core.adopt_thread(writer);

        link.listen(Target::Runtime, LISTENED);
        let mut preferences = self.preferences.with_toolkit_scrolling();
        if ephemeral {
            preferences = preferences.set("browser.privatebrowsing.autostart", true);
        }
        link.send(Target::Runtime, &preferences.defaults());
        if !self.locales.is_empty() {
            link.send(Target::Runtime, &self.locales.wire());
        }

        let watched = Arc::downgrade(&core);
        let watch = supervise::watch(launched.child, folder, move |exit| {
            if let Some(core) = watched.upgrade() {
                core.exited(exit);
            }
        })
        .map_err(Error::Spawn)?;
        core.add_watch(watch);

        let weak_core = Arc::downgrade(&core);
        let ended = Arc::downgrade(&core);
        let reader = reader::spawn(
            reading,
            move |message| {
                if let Some(core) = weak_core.upgrade() {
                    RuntimeCore::handle_message(&core, message);
                }
            },
            move |reason| {
                if let Some(core) = ended.upgrade() {
                    core.disconnected(&reason);
                }
            },
        )
        .map_err(Error::Spawn)?;
        core.adopt_thread(reader);

        if let Some(events) = nested.events() {
            let routed = Arc::downgrade(&core);
            let display = std::thread::Builder::new()
                .name("rikka-web-display".into())
                .spawn(move || {
                    for event in events {
                        match routed.upgrade() {
                            Some(core) => core.nested(event),
                            None => break,
                        }
                    }
                })
                .map_err(Error::Spawn)?;
            core.adopt_thread(display);
        }
        Ok(Runtime(core))
    }

    fn display(&self) -> Result<Nested> {
        let mut builder = Nested::builder();
        if let Some(gpu) = self.gpu.clone() {
            builder = builder.gpu(gpu);
        }
        let clipboard = match &self.clipboard {
            Some(clipboard) => Some(clipboard.share()),
            None => Clipboard::connect()
                .inspect_err(|error| {
                    tracing::debug!(%error, "engine display has no session clipboard");
                })
                .ok(),
        };
        if let Some(clipboard) = clipboard {
            builder = builder.clipboard(clipboard);
        }
        builder
            .start()
            .map_err(|error| Error::Display(error.to_string()))
    }
}
