use std::sync::Weak;

use rikka_gecko_protocol::WindowId;
use rikka_gecko_protocol::geckoview::navigation::{
    ErrorClass, LoadDecision, OnLoadError, OnLoadRequest, OnNewSession, OpenFlags, OpenWhere,
    WebRequestError,
};

use crate::error::{Error, Result};
use crate::runtime::core::RuntimeCore;
use crate::session::certificates::FailedCertificate;
use crate::session::handle::Session;
use crate::session::popup::Popup;
use crate::session::settings::SessionSettings;
use crate::uris;
use crate::wire::Answer;

#[derive(Debug)]
pub struct LoadRequest {
    pub uri: String,
    pub target: OpenWhere,
    pub flags: OpenFlags,
    pub trigger_uri: Option<String>,
    pub user_gesture: bool,
    answer: Answer,
}

impl LoadRequest {
    pub(crate) fn from_wire(request: OnLoadRequest, answer: Answer) -> LoadRequest {
        LoadRequest {
            uri: request.uri,
            target: request.target,
            flags: request.flags,
            trigger_uri: request.trigger_uri,
            user_gesture: request.has_user_gesture,
            answer,
        }
    }

    pub fn allow(self) {
        self.answer.with(&LoadDecision::Allow);
    }

    pub fn deny(self) {
        self.answer.with(&LoadDecision::Deny);
    }
}

#[derive(Debug)]
pub struct NewSessionRequest {
    pub uri: String,
    pub popup: Option<Popup>,
    window: WindowId,
    settings: SessionSettings,
    runtime: Weak<RuntimeCore>,
    answer: Answer,
}

impl NewSessionRequest {
    pub(crate) fn from_wire(
        request: OnNewSession,
        settings: SessionSettings,
        runtime: Weak<RuntimeCore>,
        answer: Answer,
    ) -> NewSessionRequest {
        NewSessionRequest {
            uri: request.uri,
            popup: request
                .popup
                .then(|| Popup::from_features(&request.features)),
            window: request.new_session_id,
            settings,
            runtime,
            answer,
        }
    }

    pub fn open(self) -> Result<Session> {
        let runtime = self.runtime.upgrade().ok_or(Error::EngineExited)?;
        let session =
            RuntimeCore::open_session(&runtime, self.window.clone(), self.settings.clone())?;
        self.answer.with(&true);
        Ok(session)
    }

    pub fn deny(self) {
        self.answer.with(&false);
    }
}

#[derive(Debug)]
pub struct LoadError {
    pub uri: Option<String>,
    pub error: WebRequestError,
    pub code: u32,
    pub class: Option<ErrorClass>,
    pub certificate: Option<FailedCertificate>,
    answer: Answer,
}

impl LoadError {
    pub(crate) fn from_wire(
        error: OnLoadError,
        certificate: Option<FailedCertificate>,
        answer: Answer,
    ) -> LoadError {
        let certificate =
            certificate.filter(|failed| error.uri.as_deref() == Some(failed.uri.as_str()));
        LoadError {
            error: error.classify(),
            class: error.class(),
            code: error.error,
            uri: error.uri,
            certificate,
            answer,
        }
    }

    pub fn show(self, html: impl AsRef<str>) {
        self.answer.with(&uris::html(html.as_ref()));
    }

    pub fn show_nothing(self) {
        drop(self);
    }
}
