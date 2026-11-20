use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Arc;

use base64::Engine;
use futures_channel::mpsc::UnboundedReceiver;
use rikka_gecko_protocol::WindowId;
use rikka_gecko_protocol::geckoview::content::{
    ExitFullScreen, GetUserAgent, Priority, SetActive, SetFocused, SetPriorityHint,
};
use rikka_gecko_protocol::geckoview::find::{ClearMatches, DisplayMatches, FindResult};
use rikka_gecko_protocol::geckoview::navigation::{
    GoBack, GoForward, GotoHistoryIndex, LoadFlags, PurgeHistory, Reload, Stop,
};
use rikka_gecko_protocol::geckoview::session::UpdateSettings;
use rikka_gecko_protocol::geckoview::session_state::{
    ContainsFormData, FlushSessionState, RestoreState,
};
use rikka_gecko_protocol::rikka::capture::StopCapture;
use rikka_gecko_protocol::rikka::editing::{
    AddToDictionary, Edit, EditCommand, ReplaceMisspelling,
};
use rikka_gecko_protocol::rikka::user_content::{CallFunction, Evaluate};
use rikka_gecko_protocol::rikka::view::{
    Download, PermitUnload, PrintToPdf, SetZoom, Snapshot as WireSnapshot, SnapshotRegion,
};
use rikka_nested::View;
use serde_json::{Map, Value};

use crate::error::{Error, Result};
use crate::runtime::Runtime;
use crate::session::certificates::{FailedCertificate, Trust};
use crate::session::content::{UserContent, World};
use crate::session::core::SessionCore;
use crate::session::event::Event;
use crate::session::find::Find;
use crate::session::load::Load;
use crate::session::media::MediaCommand;
use crate::session::page::PageState;
use crate::session::saved::SavedState;
use crate::session::settings::SessionSettings;
use crate::storage::Partition;

#[derive(Clone, Debug)]
pub struct Session {
    core: Arc<SessionCore>,
    runtime: Runtime,
}

impl PartialEq for Session {
    fn eq(&self, other: &Session) -> bool {
        Arc::ptr_eq(&self.core, &other.core)
    }
}

impl Eq for Session {}

impl Hash for Session {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.core).hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub width: u32,
    pub height: u32,
    pub png: Vec<u8>,
}

impl Session {
    pub(crate) fn new(core: Arc<SessionCore>, runtime: Runtime) -> Session {
        Session { core, runtime }
    }

    pub(crate) fn window(&self) -> &WindowId {
        &self.core.window
    }

    pub(crate) fn view(&self) -> &View {
        &self.core.view
    }

    #[must_use]
    pub fn id(&self) -> &str {
        self.core.window.as_str()
    }

    #[must_use]
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    #[must_use]
    pub fn events(&self) -> Option<UnboundedReceiver<Event>> {
        self.core.outbox.open()
    }

    #[must_use]
    pub fn page(&self) -> PageState {
        self.core.page()
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        !self.core.has_ended()
    }

    pub fn settings(&self) -> SessionSettings {
        self.core.settings()
    }

    pub fn set_settings(&self, settings: SessionSettings) {
        let private = self.core.settings().is_private();
        let settings = settings.private(private);
        self.send(&UpdateSettings(settings.wire()));
        if let Some(size) = settings.viewport() {
            self.core.view.resize(size, 1.0);
        }
        self.core.set_settings(settings);
    }

    #[must_use]
    pub fn partition(&self) -> Partition {
        let settings = self.core.settings();
        Partition {
            context: settings.context_id().map(str::to_owned),
            private: settings.is_private(),
        }
    }

    pub fn load(&self, load: impl Into<Load>) {
        let load = load.into().wire();
        self.core.ask_load(Some(&load.uri));
        self.send(&load);
    }

    pub fn reload(&self) {
        self.send(&Reload {
            flags: LoadFlags::NONE,
        });
    }

    pub fn reload_bypassing_cache(&self) {
        self.send(&Reload {
            flags: LoadFlags::BYPASS_CACHE,
        });
    }

    pub fn stop(&self) {
        self.send(&Stop);
    }

    pub fn go_back(&self) {
        self.send(&GoBack {
            user_interaction: true,
        });
    }

    pub fn go_forward(&self) {
        self.send(&GoForward {
            user_interaction: true,
        });
    }

    pub fn go_to(&self, index: usize) {
        let index = i32::try_from(index).unwrap_or(i32::MAX);
        self.send(&GotoHistoryIndex { index });
    }

    pub fn purge_history(&self) {
        self.send(&PurgeHistory);
    }

    pub fn set_active(&self, active: bool) {
        self.send(&SetActive { active });
    }

    pub fn set_focused(&self, focused: bool) {
        self.send(&SetFocused { focused });
    }

    pub fn set_visible(&self, visible: bool) {
        self.core.view.set_visible(visible);
        self.set_active(visible);
    }

    pub fn set_high_priority(&self, high: bool) {
        let priority_hint = match high {
            true => Priority::High,
            false => Priority::Default,
        };
        self.send(&SetPriorityHint { priority_hint });
    }

    pub async fn find(&self, find: Find) -> Result<FindResult> {
        self.core.link.ask(self.core.target(), &find.wire()).await
    }

    pub fn show_matches(&self, display: DisplayMatches) {
        self.send(&display);
    }

    pub fn clear_matches(&self) {
        self.send(&ClearMatches);
    }

    #[must_use]
    pub fn state(&self) -> SavedState {
        SavedState(self.core.saved())
    }

    pub fn flush_state(&self) {
        self.send(&FlushSessionState);
    }

    pub fn restore(&self, state: &SavedState) {
        self.core.ask_load(None);
        self.send(&RestoreState(state.0.clone()));
    }

    pub async fn has_form_data(&self) -> Result<bool> {
        self.core
            .link
            .ask(self.core.target(), &ContainsFormData)
            .await
    }

    pub fn user_content(&self) -> UserContent {
        self.core.content()
    }

    pub fn set_user_content(&self, content: UserContent) {
        self.send(&content.wire());
        self.core.set_content(content);
    }

    pub async fn evaluate(&self, source: impl Into<String>, world: &World) -> Result<Value> {
        let evaluate = Evaluate {
            world: world.wire(),
            source: source.into(),
        };
        self.core.link.ask(self.core.target(), &evaluate).await
    }

    pub async fn call(
        &self,
        body: impl Into<String>,
        arguments: Map<String, Value>,
        world: &World,
    ) -> Result<Value> {
        let call = CallFunction {
            world: world.wire(),
            body: body.into(),
            arguments,
        };
        self.core.link.ask(self.core.target(), &call).await
    }

    pub fn set_zoom(&self, zoom: f64) {
        self.send(&SetZoom { zoom });
        self.core.change(|page| page.zoom = zoom);
    }

    pub async fn snapshot(&self, region: SnapshotRegion, scale: Option<f64>) -> Result<Snapshot> {
        let image = self
            .core
            .link
            .ask(self.core.target(), &WireSnapshot { region, scale })
            .await?;
        let png = base64::engine::general_purpose::STANDARD
            .decode(image.png)
            .map_err(|error| Error::Malformed(error.to_string()))?;
        Ok(Snapshot {
            width: image.width,
            height: image.height,
            png,
        })
    }

    pub async fn print_to_pdf(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if !path.is_absolute() {
            return Err(Error::RelativePath(path.to_owned()));
        }
        let print = PrintToPdf {
            path: path.to_string_lossy().into_owned(),
        };
        self.core.link.ask(self.core.target(), &print).await
    }

    pub fn exit_fullscreen(&self) {
        self.send(&ExitFullScreen);
    }

    pub fn media(&self, command: MediaCommand) {
        command.send(&self.core.link, self.core.target());
    }

    pub async fn user_agent(&self) -> Result<String> {
        self.core.link.ask(self.core.target(), &GetUserAgent).await
    }

    pub async fn permit_unload(&self) -> Result<bool> {
        self.core.link.ask(self.core.target(), &PermitUnload).await
    }

    pub async fn allow_certificate(
        &self,
        certificate: &FailedCertificate,
        trust: Trust,
    ) -> Result<()> {
        self.core
            .link
            .ask(self.core.target(), &certificate.exception(trust))
            .await?;
        self.reload();
        Ok(())
    }

    pub async fn download(&self, uri: impl Into<String>) -> Result<()> {
        let download = Download { uri: uri.into() };
        self.core.link.ask(self.core.target(), &download).await
    }

    pub async fn edit(&self, command: EditCommand) -> Result<()> {
        self.core
            .link
            .ask(self.core.target(), &Edit { command })
            .await
    }

    pub fn replace_misspelling(&self, suggestion: impl Into<String>) {
        self.send(&ReplaceMisspelling {
            suggestion: suggestion.into(),
        });
    }

    pub fn add_to_dictionary(&self) {
        self.send(&AddToDictionary);
    }

    pub fn stop_capture(&self) {
        self.send(&StopCapture { only_screen: false });
    }

    pub fn stop_screen_sharing(&self) {
        self.send(&StopCapture { only_screen: true });
    }

    pub fn close(&self) {
        self.core.close();
    }

    fn send<E: rikka_gecko_protocol::ForGecko>(&self, payload: &E) {
        self.core.link.send(self.core.target(), payload);
    }
}
