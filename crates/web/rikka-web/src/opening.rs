use std::sync::Weak;

use crate::error::{Error, Result};
use crate::ids::{TabId, WindowId};
use crate::runtime::{Core, Runtime};
use crate::session::{Session, SessionSettings};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opening {
    ForegroundTab,
    BackgroundTab,
    Popup {
        width: Option<u32>,
        height: Option<u32>,
    },
    Window,
}

pub struct NewSessionRequest {
    pub url: String,
    pub title: String,
    pub opening: Opening,
    pub(crate) tab: TabId,
    pub(crate) window: WindowId,
    core: Option<Weak<Core>>,
}

impl NewSessionRequest {
    pub(crate) fn new(
        core: Weak<Core>,
        tab: TabId,
        window: WindowId,
        opening: Opening,
        url: String,
        title: String,
    ) -> NewSessionRequest {
        NewSessionRequest {
            url,
            title,
            opening,
            tab,
            window,
            core: Some(core),
        }
    }

    pub fn open(mut self, settings: SessionSettings) -> Result<Session> {
        let core = self
            .core
            .take()
            .and_then(|core| core.upgrade())
            .ok_or(Error::EngineExited)?;
        Runtime::from_core(core).adopt(self.tab, self.window, self.opening, settings)
    }

    pub fn deny(mut self) {
        self.refuse();
    }

    fn refuse(&mut self) {
        if let Some(core) = self.core.take().and_then(|core| core.upgrade()) {
            core.refuse(self.tab);
        }
    }
}

impl Drop for NewSessionRequest {
    fn drop(&mut self) {
        self.refuse();
    }
}

impl std::fmt::Debug for NewSessionRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NewSessionRequest")
            .field("url", &self.url)
            .field("opening", &self.opening)
            .finish_non_exhaustive()
    }
}
