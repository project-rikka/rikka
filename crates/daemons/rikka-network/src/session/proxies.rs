use std::collections::HashMap;

use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

use crate::values::{Draft, RawSettings};

pub(crate) const SERVICE: &str = "org.freedesktop.NetworkManager";
pub(crate) const ROOT: &str = "/org/freedesktop";
pub(crate) const MANAGER_PATH: &str = "/org/freedesktop/NetworkManager";
pub(crate) const SETTINGS_PATH: &str = "/org/freedesktop/NetworkManager/Settings";

pub(crate) const MANAGER: &str = "org.freedesktop.NetworkManager";
pub(crate) const SETTINGS: &str = "org.freedesktop.NetworkManager.Settings";
pub(crate) const CONNECTION: &str = "org.freedesktop.NetworkManager.Settings.Connection";
pub(crate) const ACTIVE: &str = "org.freedesktop.NetworkManager.Connection.Active";
pub(crate) const DEVICE: &str = "org.freedesktop.NetworkManager.Device";
pub(crate) const WIRED: &str = "org.freedesktop.NetworkManager.Device.Wired";
pub(crate) const WIRELESS: &str = "org.freedesktop.NetworkManager.Device.Wireless";
pub(crate) const ACCESS_POINT: &str = "org.freedesktop.NetworkManager.AccessPoint";
pub(crate) const IP4: &str = "org.freedesktop.NetworkManager.IP4Config";
pub(crate) const IP6: &str = "org.freedesktop.NetworkManager.IP6Config";

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
pub(crate) trait Manager {
    fn activate_connection(
        &self,
        connection: &ObjectPath<'_>,
        device: &ObjectPath<'_>,
        specific: &ObjectPath<'_>,
    ) -> zbus::Result<OwnedObjectPath>;

    fn add_and_activate_connection(
        &self,
        settings: &Draft,
        device: &ObjectPath<'_>,
        specific: &ObjectPath<'_>,
    ) -> zbus::Result<(OwnedObjectPath, OwnedObjectPath)>;

    fn deactivate_connection(&self, active: &ObjectPath<'_>) -> zbus::Result<()>;

    fn check_connectivity(&self) -> zbus::Result<u32>;

    fn enable(&self, enable: bool) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_wireless_enabled(&self, enabled: bool) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Device",
    default_service = "org.freedesktop.NetworkManager",
    gen_async = false
)]
pub(crate) trait Device {
    fn disconnect(&self) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_autoconnect(&self, autoconnect: bool) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wireless",
    default_service = "org.freedesktop.NetworkManager",
    gen_async = false
)]
pub(crate) trait WirelessDevice {
    fn request_scan(&self, options: HashMap<&str, Value<'_>>) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.AccessPoint",
    default_service = "org.freedesktop.NetworkManager",
    gen_async = false
)]
pub(crate) trait AccessPointObject {
    #[zbus(property)]
    fn ssid(&self) -> zbus::Result<Vec<u8>>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Settings",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager/Settings"
)]
pub(crate) trait SettingsStore {
    fn add_connection(&self, settings: &Draft) -> zbus::Result<OwnedObjectPath>;

    fn add_connection2(
        &self,
        settings: &Draft,
        flags: u32,
        args: HashMap<&str, Value<'_>>,
    ) -> zbus::Result<(OwnedObjectPath, HashMap<String, OwnedValue>)>;

    fn get_connection_by_uuid(&self, uuid: &str) -> zbus::Result<OwnedObjectPath>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Settings.Connection",
    default_service = "org.freedesktop.NetworkManager"
)]
pub(crate) trait SavedConnection {
    fn get_settings(&self) -> zbus::Result<RawSettings>;

    fn get_secrets(&self, setting: &str) -> zbus::Result<RawSettings>;

    fn update(&self, settings: &Draft) -> zbus::Result<()>;

    fn delete(&self) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Connection.Active",
    default_service = "org.freedesktop.NetworkManager",
    gen_blocking = false
)]
pub(crate) trait ActiveConnection {
    #[zbus(signal)]
    fn state_changed(&self, state: u32, reason: u32) -> zbus::Result<()>;

    #[zbus(property, name = "State")]
    fn current_state(&self) -> zbus::Result<u32>;
}
