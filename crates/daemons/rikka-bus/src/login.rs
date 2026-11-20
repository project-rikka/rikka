use zbus::blocking::Connection;
use zbus::zvariant::OwnedObjectPath;

#[zbus::proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1",
    gen_async = false
)]
pub trait Manager {
    #[zbus(name = "GetSessionByPID")]
    fn get_session_by_pid(&self, pid: u32) -> zbus::Result<OwnedObjectPath>;

    fn power_off(&self, interactive: bool) -> zbus::Result<()>;

    fn reboot(&self, interactive: bool) -> zbus::Result<()>;

    fn suspend(&self, interactive: bool) -> zbus::Result<()>;

    fn can_power_off(&self) -> zbus::Result<String>;

    fn can_reboot(&self) -> zbus::Result<String>;

    fn can_suspend(&self) -> zbus::Result<String>;

    fn can_hibernate(&self) -> zbus::Result<String>;

    fn inhibit(
        &self,
        what: &str,
        who: &str,
        why: &str,
        mode: &str,
    ) -> zbus::Result<zbus::zvariant::OwnedFd>;
}

#[zbus::proxy(
    interface = "org.freedesktop.login1.Session",
    default_service = "org.freedesktop.login1",
    gen_async = false
)]
pub trait Session {
    fn set_brightness(&self, subsystem: &str, name: &str, brightness: u32) -> zbus::Result<()>;

    fn set_locked_hint(&self, locked: bool) -> zbus::Result<()>;

    #[zbus(property)]
    fn id(&self) -> zbus::Result<String>;
}

pub fn current_session(manager: &ManagerProxy<'_>) -> zbus::Result<OwnedObjectPath> {
    manager.get_session_by_pid(std::process::id())
}

pub fn session_proxy(
    connection: &Connection,
    path: OwnedObjectPath,
) -> zbus::Result<SessionProxy<'static>> {
    SessionProxy::builder(connection)
        .path(path)?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
}
