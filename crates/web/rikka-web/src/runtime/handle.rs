use std::sync::Arc;

use futures_channel::mpsc::UnboundedReceiver;
use rikka_gecko_protocol::Target;
use rikka_gecko_protocol::rikka::network::Preconnect;

use crate::downloads::Downloads;
use crate::error::Result;
use crate::extensions::Extensions;
use crate::preferences::{Locales, Preferences};
use crate::runtime::builder::RuntimeBuilder;
use crate::runtime::core::RuntimeCore;
use crate::runtime::event::RuntimeEvent;
use crate::session::{Session, SessionSettings};
use crate::storage::Storage;

#[derive(Clone, Debug)]
pub struct Runtime(pub(crate) Arc<RuntimeCore>);

impl Runtime {
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::default()
    }

    #[must_use]
    pub fn events(&self) -> Option<UnboundedReceiver<RuntimeEvent>> {
        self.0.outbox.open()
    }

    pub fn session(&self, settings: SessionSettings) -> Result<Session> {
        let window = self.0.next_window_id();
        RuntimeCore::open_session(&self.0, window, settings)
    }

    #[must_use]
    pub fn extensions(&self) -> Extensions {
        Extensions::new(&self.0.link)
    }

    #[must_use]
    pub fn storage(&self) -> Storage {
        Storage::new(&self.0.link)
    }

    #[must_use]
    pub fn downloads(&self) -> Downloads {
        Downloads::new(&self.0.link)
    }

    pub fn set_preferences(&self, preferences: &Preferences) {
        self.0.link.send(Target::Runtime, &preferences.defaults());
    }

    pub fn set_locales(&self, locales: &Locales) {
        self.0.link.send(Target::Runtime, &locales.wire());
    }

    pub fn preconnect(&self, uri: impl Into<String>) {
        self.0.link.send(
            Target::Runtime,
            &Preconnect {
                uri: uri.into(),
                private: self.0.ephemeral,
            },
        );
    }

    #[must_use]
    pub fn version(&self) -> Option<String> {
        self.0.version()
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        !self.0.link.is_disconnected()
    }

    #[must_use]
    pub fn is_ephemeral(&self) -> bool {
        self.0.ephemeral
    }
}
