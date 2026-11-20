use rikka_gecko_protocol::geckoview::content_blocking::ContentBlockingEvent;
use rikka_gecko_protocol::geckoview::prompt::PromptId;
use rikka_gecko_protocol::rikka::dropdown::{DropdownId, UpdateDropdown};
use rikka_gecko_protocol::rikka::view::Tooltip;

use crate::downloads::DownloadRequest;
use crate::extensions::{ActionChange, PopupOpening, TabCloseRequest, TabUpdateRequest};
use crate::session::dropdown::Dropdown;
use crate::session::hang::Hang;
use crate::session::history::{HostVisitedQuery, Visit, VisitedQuery};
use crate::session::links::OpenLink;
use crate::session::menu::ContextMenu;
use crate::session::navigation::{LoadError, LoadRequest, NewSessionRequest};
use crate::session::permissions::{MediaRequest, PermissionRequest};
use crate::session::prompts::Prompt;
use crate::session::script::ScriptMessage;

#[derive(Debug)]
#[non_exhaustive]
pub enum Event {
    Changed,
    PageStarted(String),
    PageStopped { success: bool },
    LoadRequest(LoadRequest),
    NewSession(NewSessionRequest),
    OpenLink(OpenLink),
    LoadError(LoadError),
    Prompt(Prompt),
    PromptUpdated(Prompt),
    PromptDismissed(PromptId),
    Permission(PermissionRequest),
    Media(MediaRequest),
    ContextMenu(ContextMenu),
    Dropdown(Dropdown),
    DropdownUpdated(UpdateDropdown),
    DropdownHidden(DropdownId),
    Fullscreen(bool),
    FocusRequested,
    CloseRequested,
    ContentCrashed,
    ContentKilled,
    Hang(Hang),
    Visit(Visit),
    VisitedQuery(VisitedQuery),
    HostVisitedQuery(HostVisitedQuery),
    Download(DownloadRequest),
    ScriptMessage(ScriptMessage),
    Blocked(ContentBlockingEvent),
    Tooltip(Option<Tooltip>),
    ExtensionAction(ActionChange),
    ExtensionPopup(PopupOpening),
    ExtensionTabUpdate(TabUpdateRequest),
    ExtensionTabClose(TabCloseRequest),
    Closed(CloseReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CloseReason {
    Requested,
    EngineExited,
}
