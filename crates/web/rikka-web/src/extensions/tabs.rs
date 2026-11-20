use std::sync::Weak;

use rikka_gecko_protocol::WindowId;
use rikka_gecko_protocol::geckoview::webextension::tab::{CreateTabDetails, UpdateTabDetails};

use crate::error::{Error, Result};
use crate::runtime::core::RuntimeCore;
use crate::session::{Session, SessionSettings};
use crate::wire::Answer;

#[derive(Debug)]
pub struct NewTabRequest {
    pub extension_id: String,
    pub details: CreateTabDetails,
    window: WindowId,
    runtime: Weak<RuntimeCore>,
    answer: Answer,
}

impl NewTabRequest {
    pub(crate) fn new(
        extension_id: String,
        details: CreateTabDetails,
        window: WindowId,
        runtime: Weak<RuntimeCore>,
        answer: Answer,
    ) -> NewTabRequest {
        NewTabRequest {
            extension_id,
            details,
            window,
            runtime,
            answer,
        }
    }

    pub fn open(self, settings: SessionSettings) -> Result<Session> {
        let runtime = self.runtime.upgrade().ok_or(Error::EngineExited)?;
        let session = RuntimeCore::open_session(&runtime, self.window.clone(), settings)?;
        self.answer.with(&true);
        Ok(session)
    }

    pub fn deny(self) {
        self.answer.with(&false);
    }
}

#[derive(Debug)]
pub struct TabUpdateRequest {
    pub extension_id: String,
    pub details: UpdateTabDetails,
    answer: Answer,
}

impl TabUpdateRequest {
    pub(crate) fn new(
        extension_id: String,
        details: UpdateTabDetails,
        answer: Answer,
    ) -> TabUpdateRequest {
        TabUpdateRequest {
            extension_id,
            details,
            answer,
        }
    }

    pub fn complete(self) {
        self.answer.with(&());
    }

    pub fn deny(self) {
        drop(self);
    }
}

#[derive(Debug)]
pub struct TabCloseRequest {
    pub extension_id: String,
    answer: Answer,
}

impl TabCloseRequest {
    pub(crate) fn new(extension_id: String, answer: Answer) -> TabCloseRequest {
        TabCloseRequest {
            extension_id,
            answer,
        }
    }

    pub fn complete(self) {
        self.answer.with(&());
    }

    pub fn deny(self) {
        drop(self);
    }
}

#[derive(Debug)]
pub struct OptionsPageRequest {
    pub extension_id: String,
    answer: Answer,
}

impl OptionsPageRequest {
    pub(crate) fn new(extension_id: String, answer: Answer) -> OptionsPageRequest {
        OptionsPageRequest {
            extension_id,
            answer,
        }
    }

    pub fn complete(self) {
        self.answer.with(&());
    }

    pub fn deny(self) {
        drop(self);
    }
}
