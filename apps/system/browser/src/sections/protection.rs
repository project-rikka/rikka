use browser_engine_settings::Permission;
use rikka_app::prelude::*;
use rikka_types::Choice;
use rikka_web::{CookiePolicy, PermissionDefault, TrackingProtection};
use rikka_widgets::column;

use crate::sections::engine::{CHOICE, change, choice_field, current, title};
use crate::shell::Shell;

fn tracking_name(level: TrackingProtection) -> Label {
    match level {
        TrackingProtection::Standard => tr!("browser.protection.tracking-standard"),
        TrackingProtection::Strict => tr!("browser.protection.tracking-strict"),
        TrackingProtection::Off => tr!("browser.protection.tracking-off"),
    }
}

fn cookies_name(policy: CookiePolicy) -> Label {
    match policy {
        CookiePolicy::PartitionThirdParty => tr!("browser.protection.cookies-partition"),
        CookiePolicy::RejectTrackers => tr!("browser.protection.cookies-reject-trackers"),
        CookiePolicy::RejectThirdParty => tr!("browser.protection.cookies-reject-third-party"),
        CookiePolicy::AcceptVisited => tr!("browser.protection.cookies-accept-visited"),
        CookiePolicy::RejectAll => tr!("browser.protection.cookies-reject-all"),
        CookiePolicy::AcceptAll => tr!("browser.protection.cookies-accept-all"),
    }
}

fn permission_name(permission: Permission) -> Label {
    match permission {
        Permission::Location => tr!("browser.site.location"),
        Permission::Camera => tr!("browser.site.camera"),
        Permission::Microphone => tr!("browser.site.microphone"),
        Permission::ScreenSharing => tr!("browser.site.screen"),
        Permission::Notifications => tr!("browser.site.notifications"),
        Permission::ProtectedMedia => tr!("browser.site.protected-media"),
    }
}

fn default_name(default: PermissionDefault) -> Label {
    match default {
        PermissionDefault::Ask => tr!("browser.protection.ask"),
        PermissionDefault::Block => tr!("browser.protection.block"),
    }
}

pub(crate) fn section_view(shell: &Shell) -> Element {
    let settings = current(shell);
    section(
        title(shell, tr!("browser.protection.title")),
        column! {
            choice_field(
                tr!("browser.protection.tracking"),
                settings.tracking_protection,
                tracking_name,
                |settings, level| settings.tracking_protection = level,
            );
            choice_field(
                tr!("browser.protection.cookies"),
                settings.cookies,
                cookies_name,
                |settings, policy| settings.cookies = policy,
            );
        }
        .gap(Space::NORMAL)
        .fill_width(),
    )
    .any()
}

pub(crate) fn permissions_view(shell: &Shell) -> Element {
    let defaults = current(shell).permissions;
    let rows = Permission::ALL.map(|permission| {
        let chosen = defaults.get(permission);
        field(
            permission_name(permission),
            combo(
                PermissionDefault::ALL
                    .iter()
                    .position(|candidate| *candidate == chosen),
            )
            .options(PermissionDefault::ALL.iter().copied().map(default_name))
            .on_select(move |shell: &mut Shell, at: usize| {
                if let Some(default) = PermissionDefault::ALL.get(at).copied() {
                    change(shell, |settings| {
                        settings.permissions = settings.permissions.with(permission, default);
                    });
                }
            })
            .width(CHOICE),
        )
        .any()
    });
    page([section(
        title(shell, tr!("browser.protection.permissions")),
        column_of(rows).gap(Space::NORMAL).fill_width(),
    )])
    .any()
}
