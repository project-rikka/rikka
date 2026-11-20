use rikka_gecko_protocol::rikka::downloads::DownloadId;

use crate::downloads::Download;
use crate::extensions::{
    ActionChange, ExtensionChange, InstallFailure, InstallRequest, NewTabRequest,
    OptionalPermissionsRequest, OptionsPageRequest, UpdateRequest,
};
use crate::notifications::Notification;
use crate::process::supervise::Exit;
use crate::runtime::open_window::OpenWindowRequest;

#[derive(Debug)]
#[non_exhaustive]
pub enum RuntimeEvent {
    Started { version: String },
    Incompatible { protocol: u32 },
    Exited(Exit),
    Download(Download),
    DownloadRemoved(DownloadId),
    Notification(Notification),
    NotificationWithdrawn(String),
    ExtensionInstall(InstallRequest),
    ExtensionUpdate(UpdateRequest),
    ExtensionPermissions(OptionalPermissionsRequest),
    Extension(ExtensionChange),
    ExtensionInstallFailed(InstallFailure),
    ExtensionAction(ActionChange),
    ExtensionTab(NewTabRequest),
    ExtensionOptions(OptionsPageRequest),
    OpenWindow(OpenWindowRequest),
}
