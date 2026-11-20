pub(crate) const SERVICE: &str = "org.freedesktop.UPower";

pub(crate) const DEVICE: &str = "org.freedesktop.UPower.Device";

pub(crate) const MANAGER_PATH: &str = "/org/freedesktop/UPower";

pub(crate) const DISPLAY_DEVICE: &str = "/org/freedesktop/UPower/devices/DisplayDevice";

pub(crate) const BATTERY: u32 = 2;

#[zbus::proxy(
    interface = "org.freedesktop.UPower",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower",
    gen_async = false
)]
pub(crate) trait Manager {
    fn enumerate_devices(&self) -> zbus::Result<Vec<zbus::zvariant::OwnedObjectPath>>;

    #[zbus(property)]
    fn on_battery(&self) -> zbus::Result<bool>;
}

#[zbus::proxy(
    interface = "org.freedesktop.UPower.Device",
    default_service = "org.freedesktop.UPower",
    gen_async = false
)]
pub(crate) trait Device {
    fn get_history(
        &self,
        kind: &str,
        timespan: u32,
        resolution: u32,
    ) -> zbus::Result<Vec<(u32, f64, u32)>>;

    fn enable_charge_threshold(&self, enabled: bool) -> zbus::Result<()>;
}
