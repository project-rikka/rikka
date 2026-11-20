use std::sync::Weak;

use rikka_gecko_protocol::geckoview::runtime::OpenedWindow;

use crate::error::{Error, Result};
use crate::runtime::core::RuntimeCore;
use crate::session::{Session, SessionSettings};
use crate::wire::Answer;

#[derive(Debug)]
pub struct OpenWindowRequest {
    pub uri: String,
    runtime: Weak<RuntimeCore>,
    answer: Answer,
}

impl OpenWindowRequest {
    pub(crate) fn new(
        uri: String,
        runtime: Weak<RuntimeCore>,
        answer: Answer,
    ) -> OpenWindowRequest {
        OpenWindowRequest {
            uri,
            runtime,
            answer,
        }
    }

    pub fn open(self, settings: SessionSettings) -> Result<Session> {
        let runtime = self.runtime.upgrade().ok_or(Error::EngineExited)?;
        let window = runtime.next_window_id();
        self.answer.with(&Some(OpenedWindow {
            is_open: false,
            session_id: window.clone(),
        }));
        RuntimeCore::open_session(&runtime, window, settings)
    }

    pub fn deny(self) {
        self.answer.with(&None::<OpenedWindow>);
    }
}
