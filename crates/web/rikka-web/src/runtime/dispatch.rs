use std::sync::Arc;

use rikka_gecko_protocol::event::Event as _;
use rikka_gecko_protocol::geckoview::runtime::{OpenedWindow, ServiceWorkerOpenWindow};
use rikka_gecko_protocol::geckoview::webextension::action::{
    BrowserActionUpdate, PageActionUpdate,
};
use rikka_gecko_protocol::geckoview::webextension::extension::Extension;
use rikka_gecko_protocol::geckoview::webextension::lifecycle::{
    OnDisabled, OnDisabling, OnEnabled, OnEnabling, OnInstallationFailed, OnInstalled,
    OnInstalling, OnOptionalPermissionsChanged, OnReady, OnUninstalled, OnUninstalling,
};
use rikka_gecko_protocol::geckoview::webextension::prompt::{
    AllowResponse, InstallPrompt, InstallPromptResponse, OptionalPrompt, UpdatePrompt,
};
use rikka_gecko_protocol::geckoview::webextension::tab::{NewTab, OpenOptionsPage};
use rikka_gecko_protocol::inbound::RuntimeEvent as Inbound;
use rikka_gecko_protocol::rikka::downloads::{DownloadChanged, DownloadRemoved};
use rikka_gecko_protocol::rikka::notifications::{CloseNotification, ShowNotification};
use rikka_gecko_protocol::{CallbackId, EventMessage};
use serde_json::Value;

use crate::downloads::Download;
use crate::extensions::{
    ActionChange, ActionKind, ExtensionChange, InstallFailure, InstallRequest, Lifecycle,
    NewTabRequest, OptionalPermissionsRequest, OptionsPageRequest, UpdateRequest, attach_actions,
};
use crate::notifications::Notification;
use crate::runtime::core::RuntimeCore;
use crate::runtime::event::RuntimeEvent;
use crate::runtime::open_window::OpenWindowRequest;
use crate::wire::Answer;

pub(crate) const LISTENED: &[&str] = &[
    ServiceWorkerOpenWindow::NAME,
    InstallPrompt::NAME,
    UpdatePrompt::NAME,
    OptionalPrompt::NAME,
    OnInstalling::NAME,
    OnInstalled::NAME,
    OnUninstalling::NAME,
    OnUninstalled::NAME,
    OnEnabling::NAME,
    OnEnabled::NAME,
    OnDisabling::NAME,
    OnDisabled::NAME,
    OnReady::NAME,
    OnOptionalPermissionsChanged::NAME,
    OnInstallationFailed::NAME,
    BrowserActionUpdate::NAME,
    PageActionUpdate::NAME,
    NewTab::NAME,
    OpenOptionsPage::NAME,
    DownloadChanged::NAME,
    DownloadRemoved::NAME,
    ShowNotification::NAME,
    CloseNotification::NAME,
];

pub(crate) fn handle_message(core: &Arc<RuntimeCore>, message: &EventMessage) {
    let callback = message.callback;
    match Inbound::decode(message) {
        Ok(Some(event)) => dispatch(core, event, callback),
        Ok(None) => core.link.refuse(callback, "unhandled event"),
        Err(error) => {
            tracing::warn!(%error, event = %message.event, "failed to decode event");
            core.link.refuse(callback, "malformed event");
        }
    }
}

fn dispatch(core: &Arc<RuntimeCore>, event: Inbound, callback: Option<CallbackId>) {
    let link = &core.link;
    let refusing = |reason: &str| Answer::new(link, callback, Err(Value::String(reason.into())));
    let send = |event: RuntimeEvent| core.outbox.send(event);
    let changed = |lifecycle: Lifecycle, extension: Extension| {
        send(RuntimeEvent::Extension(ExtensionChange {
            lifecycle,
            extension,
        }));
    };
    match event {
        Inbound::ServiceWorkerOpenWindow(open) => {
            send(RuntimeEvent::OpenWindow(OpenWindowRequest::new(
                open.url,
                Arc::downgrade(core),
                Answer::fallback_to(link, callback, &None::<OpenedWindow>),
            )))
        }
        Inbound::InstallPrompt(prompt) => {
            send(RuntimeEvent::ExtensionInstall(InstallRequest::from_wire(
                prompt,
                Answer::fallback_to(link, callback, &InstallPromptResponse::default()),
            )))
        }
        Inbound::ExtensionUpdatePrompt(prompt) => {
            send(RuntimeEvent::ExtensionUpdate(UpdateRequest::from_wire(
                prompt,
                Answer::fallback_to(link, callback, &AllowResponse { allow: false }),
            )))
        }
        Inbound::OptionalPrompt(prompt) => send(RuntimeEvent::ExtensionPermissions(
            OptionalPermissionsRequest::from_wire(
                prompt,
                Answer::fallback_to(link, callback, &AllowResponse { allow: false }),
            ),
        )),
        Inbound::OnInstalling(notice) => changed(Lifecycle::Installing, notice.extension),
        Inbound::OnInstalled(notice) => {
            attach_actions(link, &notice.extension.web_extension_id);
            changed(Lifecycle::Installed, notice.extension);
        }
        Inbound::OnUninstalling(notice) => changed(Lifecycle::Uninstalling, notice.extension),
        Inbound::OnUninstalled(notice) => changed(Lifecycle::Uninstalled, notice.extension),
        Inbound::OnEnabling(notice) => changed(Lifecycle::Enabling, notice.extension),
        Inbound::OnEnabled(notice) => {
            attach_actions(link, &notice.extension.web_extension_id);
            changed(Lifecycle::Enabled, notice.extension);
        }
        Inbound::OnDisabling(notice) => changed(Lifecycle::Disabling, notice.extension),
        Inbound::OnDisabled(notice) => changed(Lifecycle::Disabled, notice.extension),
        Inbound::OnReady(notice) => {
            attach_actions(link, &notice.extension.web_extension_id);
            changed(Lifecycle::Ready, notice.extension);
        }
        Inbound::OnOptionalPermissionsChanged(notice) => {
            changed(Lifecycle::PermissionsChanged, notice.extension);
        }
        Inbound::OnInstallationFailed(failed) => {
            send(RuntimeEvent::ExtensionInstallFailed(
                InstallFailure::from_wire(failed),
            ));
        }
        Inbound::BrowserActionUpdate(update) => send(RuntimeEvent::ExtensionAction(
            ActionChange::from_wire(ActionKind::Browser, update.0),
        )),
        Inbound::PageActionUpdate(update) => send(RuntimeEvent::ExtensionAction(
            ActionChange::from_wire(ActionKind::Page, update.0),
        )),
        Inbound::NewTab(tab) => send(RuntimeEvent::ExtensionTab(NewTabRequest::new(
            tab.extension_id,
            tab.create_properties,
            tab.new_session_id,
            Arc::downgrade(core),
            Answer::fallback_to(link, callback, &false),
        ))),
        Inbound::OpenOptionsPage(open) => send(RuntimeEvent::ExtensionOptions(
            OptionsPageRequest::new(open.extension_id, refusing("options pages are not shown")),
        )),
        Inbound::DownloadChanged(changed) => {
            send(RuntimeEvent::Download(Download::from_wire(changed)))
        }
        Inbound::DownloadRemoved(removed) => send(RuntimeEvent::DownloadRemoved(removed.id)),
        Inbound::ShowNotification(shown) => {
            send(RuntimeEvent::Notification(Notification::from_wire(
                shown, link,
            )));
        }
        Inbound::CloseNotification(closed) => {
            send(RuntimeEvent::NotificationWithdrawn(closed.name));
        }
        _ => core.link.refuse(callback, "unhandled event"),
    }
}
