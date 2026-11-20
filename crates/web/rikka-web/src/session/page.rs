use std::collections::BTreeMap;
use std::sync::Arc;

use rikka_gecko_protocol::geckoview::media::{MediaMetadata, PositionState};
use rikka_gecko_protocol::geckoview::progress::{IdentityMode, MixedMode, SecurityIdentity};
use rikka_gecko_protocol::geckoview::session_state::SessionState;
use rikka_gecko_protocol::rikka::capture::{CaptureChanged, CaptureState};
use rikka_gecko_protocol::rikka::view::Icon;

#[derive(Clone, Debug, PartialEq)]
pub struct PageState {
    pub uri: String,
    pub title: String,
    pub progress: f64,
    pub loading: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub history: Arc<[HistoryEntry]>,
    pub history_index: Option<usize>,
    pub security: Arc<Security>,
    pub fullscreen: bool,
    pub hovered_link: Option<String>,
    pub icons: Arc<[Icon]>,
    pub zoom: f64,
    pub painted: bool,
    pub media: MediaState,
    pub camera: bool,
    pub microphone: bool,
    pub capture: MediaCapture,
}

impl Default for PageState {
    fn default() -> PageState {
        PageState {
            uri: String::new(),
            title: String::new(),
            progress: 0.0,
            loading: false,
            can_go_back: false,
            can_go_forward: false,
            history: Arc::default(),
            history_index: None,
            security: Arc::default(),
            fullscreen: false,
            hovered_link: None,
            icons: Arc::default(),
            zoom: 1.0,
            painted: false,
            media: MediaState::default(),
            camera: false,
            microphone: false,
            capture: MediaCapture::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MediaCapture {
    pub camera: CaptureState,
    pub microphone: CaptureState,
    pub screen: CaptureState,
}

impl MediaCapture {
    pub(crate) fn from_wire(changed: CaptureChanged) -> MediaCapture {
        MediaCapture {
            camera: changed.camera,
            microphone: changed.microphone,
            screen: changed.screen,
        }
    }
}

impl PageState {
    pub(crate) fn follow_history(&mut self, state: &SessionState) {
        self.history = state
            .entries()
            .iter()
            .map(|entry| HistoryEntry {
                uri: entry.url.clone(),
                title: entry.title.clone().unwrap_or_default(),
            })
            .collect();
        self.history_index = state.current_index();
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HistoryEntry {
    pub uri: String,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Security {
    pub secure: bool,
    pub identity: IdentityMode,
    pub mixed_display: MixedMode,
    pub mixed_active: MixedMode,
    pub origin: Option<String>,
    pub host: Option<String>,
    pub certificate: Option<String>,
    pub exception: bool,
}

impl Default for Security {
    fn default() -> Security {
        Security {
            secure: false,
            identity: IdentityMode::Unknown,
            mixed_display: MixedMode::Unknown,
            mixed_active: MixedMode::Unknown,
            origin: None,
            host: None,
            certificate: None,
            exception: false,
        }
    }
}

impl Security {
    pub(crate) fn from_wire(identity: SecurityIdentity) -> Security {
        Security {
            secure: identity.secure,
            identity: identity.mode.identity,
            mixed_display: identity.mode.mixed_display,
            mixed_active: identity.mode.mixed_active,
            origin: identity.origin,
            host: identity.host,
            certificate: identity.certificate,
            exception: identity.security_exception.unwrap_or(false),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Playback {
    #[default]
    None,
    Paused,
    Playing,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaState {
    pub active: bool,
    pub playback: Playback,
    pub metadata: Option<MediaMetadata>,
    pub position: Option<PositionState>,
    pub features: BTreeMap<String, bool>,
}
