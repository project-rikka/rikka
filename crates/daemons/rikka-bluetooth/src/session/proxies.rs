use std::collections::HashMap;

use zbus::zvariant::{ObjectPath, Value};

pub(crate) const SERVICE: &str = "org.bluez";
pub(crate) const ROOT: &str = "/";

pub(crate) const ADAPTER: &str = "org.bluez.Adapter1";
pub(crate) const DEVICE: &str = "org.bluez.Device1";
pub(crate) const BATTERY: &str = "org.bluez.Battery1";

#[zbus::proxy(
    interface = "org.bluez.AgentManager1",
    default_service = "org.bluez",
    default_path = "/org/bluez",
    gen_async = false
)]
pub(crate) trait AgentManager {
    fn register_agent(&self, agent: &ObjectPath<'_>, capability: &str) -> zbus::Result<()>;

    fn request_default_agent(&self, agent: &ObjectPath<'_>) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.bluez.Adapter1",
    default_service = "org.bluez",
    gen_async = false
)]
pub(crate) trait Adapter {
    fn start_discovery(&self) -> zbus::Result<()>;

    fn stop_discovery(&self) -> zbus::Result<()>;

    fn set_discovery_filter(&self, filter: HashMap<&str, Value<'_>>) -> zbus::Result<()>;

    fn remove_device(&self, device: &ObjectPath<'_>) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_powered(&self, powered: bool) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_discoverable(&self, discoverable: bool) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_pairable(&self, pairable: bool) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_alias(&self, alias: &str) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.bluez.Device1",
    default_service = "org.bluez",
    gen_async = false
)]
pub(crate) trait Device {
    fn connect(&self) -> zbus::Result<()>;

    fn disconnect(&self) -> zbus::Result<()>;

    fn pair(&self) -> zbus::Result<()>;

    fn cancel_pairing(&self) -> zbus::Result<()>;

    #[zbus(property)]
    fn adapter(&self) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;

    #[zbus(property)]
    fn set_trusted(&self, trusted: bool) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_blocked(&self, blocked: bool) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_alias(&self, alias: &str) -> zbus::Result<()>;
}
