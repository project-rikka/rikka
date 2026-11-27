use crate::api::{Account, DownloadItem, HistoryItem};
use crate::context_menu::ContextMenu;
use crate::foreign::ForeignWindow;
use crate::opening::NewSessionRequest;
use crate::page::Transition;
use crate::process::supervise::Exit;
use crate::script::{ScriptAsk, ScriptMessage};

#[derive(Debug)]
#[non_exhaustive]
pub enum Event {
    Changed,
    Started { url: String },
    Committed { url: String, transition: Transition },
    Finished { url: String },
    Failed { url: String, error: String },
    NewSession(NewSessionRequest),
    Fullscreen(bool),
    ActivationRequested,
    ContextMenu(ContextMenu),
    Message(ScriptMessage),
    Ask(ScriptAsk),
    Closed(CloseReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CloseReason {
    Requested,
    Page,
    EngineExited,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    ExtensionMissing,
    ExtensionOutdated { found: u32, expected: u32 },
}

#[derive(Debug)]
#[non_exhaustive]
pub enum RuntimeEvent {
    Ready { version: String },
    Failed(Failure),
    Exited(Exit),
    NewSession(NewSessionRequest),
    ForeignWindow(ForeignWindow),
    Downloads(Vec<DownloadItem>),
    Visited(HistoryItem),
    VisitsRemoved { all: bool, urls: Vec<String> },
    BookmarksChanged,
    ExtensionsChanged,
    AccountChanged(Option<Account>),
}
