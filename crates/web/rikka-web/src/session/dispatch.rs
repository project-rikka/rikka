use std::sync::Arc;

use rikka_gecko_protocol::event::Event as _;
use rikka_gecko_protocol::geckoview::content::{
    ContentCrash, ContentKill, ContextMenu as WireMenu, FirstContentfulPaint, FocusRequest,
    HangReport, PageTitleChanged, PaintStatusReset, WindowCloseRequest,
};
use rikka_gecko_protocol::geckoview::content_blocking::ContentBlockingEvent;
use rikka_gecko_protocol::geckoview::history::{GetHostVisitedSince, GetVisited, OnVisited};
use rikka_gecko_protocol::geckoview::media::{
    FeaturesChanged, MediaSessionActivated, MediaSessionDeactivated, MetadataChanged, PlaybackNone,
    PlaybackPaused, PlaybackPlaying, PositionStateChanged, RecordingDeviceType, RecordingStatus,
    RecordingStatusChanged,
};
use rikka_gecko_protocol::geckoview::navigation::{
    LoadDecision, LocationChange, OnLoadError, OnLoadRequest, OnNewSession,
};
use rikka_gecko_protocol::geckoview::permission::{
    AppPermissionRequest, ContentPermissionRequest, MediaPermissionDecision,
    MediaPermissionRequest, PermissionValue,
};
use rikka_gecko_protocol::geckoview::progress::{
    PageStart, PageStop, ProgressChanged, SecurityChanged,
};
use rikka_gecko_protocol::geckoview::prompt::{DismissPrompt, ShowPrompt, UpdatePrompt};
use rikka_gecko_protocol::geckoview::session_state::StateUpdated;
use rikka_gecko_protocol::geckoview::webextension::action::{
    BrowserActionOpenPopup, BrowserActionUpdate, PageActionOpenPopup, PageActionUpdate,
};
use rikka_gecko_protocol::geckoview::webextension::tab::{CloseTab, UpdateTab};
use rikka_gecko_protocol::inbound::WindowEvent;
use rikka_gecko_protocol::rikka::capture::{CaptureChanged, CaptureState};
use rikka_gecko_protocol::rikka::certificates::CertificateError;
use rikka_gecko_protocol::rikka::downloads::{DownloadRequest as WireDownload, SaveTo};
use rikka_gecko_protocol::rikka::dropdown::{
    DropdownChoice, HideDropdown, ShowDropdown, UpdateDropdown,
};
use rikka_gecko_protocol::rikka::editing::ContextMenu as RichMenu;
use rikka_gecko_protocol::rikka::links::OpenLink as WireLink;
use rikka_gecko_protocol::rikka::user_content::ScriptMessage as WireMessage;
use rikka_gecko_protocol::rikka::view::{Icons, OverLink, Tooltip, TooltipHidden};
use rikka_gecko_protocol::{CallbackId, EventMessage};
use serde::Serialize;
use serde_json::Value;

use crate::downloads::DownloadRequest;
use crate::extensions::{
    ActionChange, ActionKind, PopupOpening, TabCloseRequest, TabUpdateRequest,
};
use crate::session::certificates::FailedCertificate;
use crate::session::core::SessionCore;
use crate::session::dropdown::Dropdown;
use crate::session::event::Event;
use crate::session::hang::Hang;
use crate::session::history::{HostVisitedQuery, Visit, VisitedQuery};
use crate::session::links::OpenLink;
use crate::session::menu::ContextMenu;
use crate::session::navigation::{LoadError, LoadRequest, NewSessionRequest};
use crate::session::page::{MediaCapture, MediaState, Playback, Security};
use crate::session::permissions::{MediaRequest, PermissionRequest};
use crate::session::prompts::Prompt;
use crate::session::script::ScriptMessage;
use crate::session::settings::SessionSettings;
use crate::wire::Answer;

const HEARD: &[&str] = &[
    LocationChange::NAME,
    OnNewSession::NAME,
    OnLoadError::NAME,
    PageStart::NAME,
    PageStop::NAME,
    ProgressChanged::NAME,
    SecurityChanged::NAME,
    StateUpdated::NAME,
    PageTitleChanged::NAME,
    WireMenu::NAME,
    WindowCloseRequest::NAME,
    FocusRequest::NAME,
    ContentCrash::NAME,
    ContentKill::NAME,
    FirstContentfulPaint::NAME,
    PaintStatusReset::NAME,
    HangReport::NAME,
    ShowPrompt::NAME,
    UpdatePrompt::NAME,
    DismissPrompt::NAME,
    ContentPermissionRequest::NAME,
    AppPermissionRequest::NAME,
    MediaPermissionRequest::NAME,
    OnVisited::NAME,
    GetVisited::NAME,
    GetHostVisitedSince::NAME,
    MediaSessionActivated::NAME,
    MediaSessionDeactivated::NAME,
    MetadataChanged::NAME,
    PositionStateChanged::NAME,
    FeaturesChanged::NAME,
    PlaybackNone::NAME,
    PlaybackPaused::NAME,
    PlaybackPlaying::NAME,
    RecordingStatusChanged::NAME,
    ContentBlockingEvent::NAME,
    BrowserActionUpdate::NAME,
    PageActionUpdate::NAME,
    BrowserActionOpenPopup::NAME,
    PageActionOpenPopup::NAME,
    UpdateTab::NAME,
    CloseTab::NAME,
    WireDownload::NAME,
    WireMessage::NAME,
    OverLink::NAME,
    Tooltip::NAME,
    TooltipHidden::NAME,
    Icons::NAME,
    WireLink::NAME,
    CertificateError::NAME,
    RichMenu::NAME,
    CaptureChanged::NAME,
    ShowDropdown::NAME,
    UpdateDropdown::NAME,
    HideDropdown::NAME,
];

pub(crate) fn subscribed_events(settings: &SessionSettings) -> Vec<&'static str> {
    let mut events = HEARD.to_vec();
    if settings.handles_load_requests() {
        events.push(OnLoadRequest::NAME);
    }
    events
}

pub(crate) fn handle_message(core: &SessionCore, message: &EventMessage) {
    let callback = message.callback;
    match WindowEvent::decode(message) {
        Ok(Some(event)) => dispatch(core, event, callback),
        Ok(None) => core.link.refuse(callback, "unhandled event"),
        Err(error) => {
            tracing::warn!(%error, event = %message.event, "failed to decode event");
            core.link.refuse(callback, "malformed event");
        }
    }
}

struct Reply<'a> {
    core: &'a SessionCore,
    callback: Option<CallbackId>,
}

impl Reply<'_> {
    fn or<T: Serialize>(&self, fallback: &T) -> Answer {
        Answer::fallback_to(&self.core.link, self.callback, fallback)
    }

    fn reply_or_reject(&self, reason: &str) -> Answer {
        Answer::new(
            &self.core.link,
            self.callback,
            Err(Value::String(reason.to_owned())),
        )
    }
}

fn dispatch(core: &SessionCore, event: WindowEvent, callback: Option<CallbackId>) {
    let reply = Reply { core, callback };
    let send = |event: Event| core.outbox.send(event);
    match event {
        WindowEvent::LocationChange(change) if change.is_top_level => {
            if !core.allows(&change.uri) {
                return;
            }
            core.change(|page| {
                page.uri = change.uri;
                page.can_go_back = change.can_go_back;
                page.can_go_forward = change.can_go_forward;
                page.hovered_link = None;
            });
        }
        WindowEvent::LocationChange(_) => {}
        WindowEvent::OnLoadRequest(request) => {
            let answer = reply.or(&LoadDecision::Allow);
            match core.settings().handles_load_requests() {
                true => send(Event::LoadRequest(LoadRequest::from_wire(request, answer))),
                false => answer.with(&LoadDecision::Allow),
            }
        }
        WindowEvent::OnNewSession(request) => {
            send(Event::NewSession(NewSessionRequest::from_wire(
                request,
                core.settings(),
                core.runtime.clone(),
                reply.or(&false),
            )))
        }
        WindowEvent::OnLoadError(error) => send(Event::LoadError(LoadError::from_wire(
            error,
            core.failed_certificate(),
            reply.reply_or_reject("no error page was given"),
        ))),
        WindowEvent::CertificateError(error) => {
            if let Some(certificate) = FailedCertificate::from_wire(error) {
                core.certificate_failed(certificate);
            }
        }
        WindowEvent::OpenLink(link) => send(Event::OpenLink(OpenLink::from_wire(link))),
        WindowEvent::PageStart(start) => {
            if !core.allows(&start.uri) {
                return;
            }
            core.change(|page| {
                page.loading = true;
                page.progress = 0.0;
                page.icons = Arc::default();
            });
            send(Event::PageStarted(start.uri));
        }
        WindowEvent::PageStop(_)
        | WindowEvent::ProgressChanged(_)
        | WindowEvent::SecurityChanged(_)
            if core.is_loading_initial() => {}
        WindowEvent::PageStop(stop) => {
            core.change(|page| {
                page.loading = false;
                page.progress = 1.0;
            });
            send(Event::PageStopped {
                success: stop.success,
            });
        }
        WindowEvent::ProgressChanged(progress) => {
            core.change(|page| page.progress = f64::from(progress.progress.min(100)) / 100.0);
        }
        WindowEvent::SecurityChanged(security) => {
            core.change(|page| page.security = Arc::new(Security::from_wire(security.identity)));
        }
        WindowEvent::StateUpdated(update) => core.record(update.data),
        WindowEvent::PageTitleChanged(title) => core.change(|page| page.title = title.title),
        WindowEvent::ContextMenu(menu) => send(Event::ContextMenu(ContextMenu::from_wire(menu))),
        WindowEvent::RikkaContextMenu(menu) => send(Event::ContextMenu(ContextMenu::rich(menu))),
        WindowEvent::ShowDropdown(shown) => send(Event::Dropdown(Dropdown::from_wire(
            shown,
            reply.or(&None::<DropdownChoice>),
        ))),
        WindowEvent::UpdateDropdown(updated) => send(Event::DropdownUpdated(updated)),
        WindowEvent::HideDropdown(hidden) => send(Event::DropdownHidden(hidden.id)),
        WindowEvent::WindowCloseRequest(_) => send(Event::CloseRequested),
        WindowEvent::FocusRequest(_) => send(Event::FocusRequested),
        WindowEvent::ContentCrash(_) => send(Event::ContentCrashed),
        WindowEvent::ContentKill(_) => send(Event::ContentKilled),
        WindowEvent::FirstContentfulPaint(_) => core.change(|page| page.painted = true),
        WindowEvent::PaintStatusReset(_) => core.change(|page| page.painted = false),
        WindowEvent::HangReport(report) => {
            send(Event::Hang(Hang::new(report, core.target(), &core.link)));
        }
        WindowEvent::ShowPrompt(shown) => {
            send(Event::Prompt(Prompt::from_wire(
                shown.prompt,
                reply.or(&Value::Null),
            )));
        }
        WindowEvent::UpdatePrompt(updated) => send(Event::PromptUpdated(Prompt::from_wire(
            updated.prompt,
            Answer::new(&core.link, None, Ok(Value::Null)),
        ))),
        WindowEvent::DismissPrompt(dismissed) => send(Event::PromptDismissed(dismissed.id)),
        WindowEvent::ContentPermissionRequest(request) => send(Event::Permission(
            PermissionRequest::new(request.0, core.target(), reply.or(&PermissionValue::Deny)),
        )),
        WindowEvent::AppPermissionRequest(_) => reply.or(&true).with(&true),
        WindowEvent::MediaPermissionRequest(request) => send(Event::Media(
            MediaRequest::from_wire(request, reply.or(&MediaPermissionDecision::Deny)),
        )),
        WindowEvent::OnVisited(visit) => {
            send(Event::Visit(Visit::from_wire(visit, reply.or(&false))))
        }
        WindowEvent::GetVisited(query) => send(Event::VisitedQuery(VisitedQuery::from_wire(
            query,
            reply.or(&None::<Vec<bool>>),
        ))),
        WindowEvent::GetHostVisitedSince(query) => send(Event::HostVisitedQuery(
            HostVisitedQuery::from_wire(query, reply.or(&None::<bool>)),
        )),
        WindowEvent::MediaSessionActivated(_) => core.change(|page| page.media.active = true),
        WindowEvent::MediaSessionDeactivated(_) => {
            core.change(|page| page.media = MediaState::default());
        }
        WindowEvent::MetadataChanged(changed) => {
            core.change(|page| page.media.metadata = Some(changed.metadata));
        }
        WindowEvent::PositionStateChanged(changed) => {
            core.change(|page| page.media.position = Some(changed.state));
        }
        WindowEvent::FeaturesChanged(changed) => {
            core.change(|page| page.media.features = changed.features);
        }
        WindowEvent::PlaybackNone(_) => core.change(|page| page.media.playback = Playback::None),
        WindowEvent::PlaybackPaused(_) => {
            core.change(|page| page.media.playback = Playback::Paused);
        }
        WindowEvent::PlaybackPlaying(_) => {
            core.change(|page| page.media.playback = Playback::Playing);
        }
        WindowEvent::RecordingStatusChanged(changed) => core.change(|page| {
            let recording = |kind: RecordingDeviceType| {
                changed.devices.iter().any(|device| {
                    device.device_type == kind && device.status == RecordingStatus::Recording
                })
            };
            page.camera = recording(RecordingDeviceType::Camera);
            page.microphone = recording(RecordingDeviceType::Microphone);
        }),
        WindowEvent::CaptureChanged(changed) => core.change(|page| {
            page.capture = MediaCapture::from_wire(changed);
            page.camera = page.capture.camera != CaptureState::Off;
            page.microphone = page.capture.microphone != CaptureState::Off;
        }),
        WindowEvent::ContentBlockingEvent(blocked) => send(Event::Blocked(blocked)),
        WindowEvent::BrowserActionUpdate(update) => send(Event::ExtensionAction(
            ActionChange::from_wire(ActionKind::Browser, update.0),
        )),
        WindowEvent::PageActionUpdate(update) => send(Event::ExtensionAction(
            ActionChange::from_wire(ActionKind::Page, update.0),
        )),
        WindowEvent::BrowserActionOpenPopup(popup) => send(Event::ExtensionPopup(
            PopupOpening::from_wire(ActionKind::Browser, popup.0),
        )),
        WindowEvent::PageActionOpenPopup(popup) => send(Event::ExtensionPopup(
            PopupOpening::from_wire(ActionKind::Page, popup.0),
        )),
        WindowEvent::UpdateTab(update) => send(Event::ExtensionTabUpdate(TabUpdateRequest::new(
            update.extension_id,
            update.update_properties,
            reply.reply_or_reject("the tab was not updated"),
        ))),
        WindowEvent::CloseTab(close) => send(Event::ExtensionTabClose(TabCloseRequest::new(
            close.extension_id,
            reply.reply_or_reject("the tab was not closed"),
        ))),
        WindowEvent::DownloadRequest(request) => send(Event::Download(DownloadRequest::from_wire(
            request,
            reply.or(&None::<SaveTo>),
        ))),
        WindowEvent::ScriptMessage(message) => {
            let answer = callback.map(|_| reply.reply_or_reject("the message was not answered"));
            send(Event::ScriptMessage(ScriptMessage::from_wire(
                message, answer,
            )));
        }
        WindowEvent::OverLink(link) => core.change(|page| page.hovered_link = link.url),
        WindowEvent::Tooltip(tooltip) => send(Event::Tooltip(Some(tooltip))),
        WindowEvent::TooltipHidden(_) => send(Event::Tooltip(None)),
        WindowEvent::Icons(icons) => core.change(|page| page.icons = icons.icons.into()),
        _ => core.link.refuse(callback, "unhandled event"),
    }
}
