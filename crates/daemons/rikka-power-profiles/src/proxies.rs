pub(crate) struct Address {
    pub(crate) service: &'static str,
    pub(crate) path: &'static str,
    pub(crate) interface: &'static str,
}

pub(crate) static ADDRESSES: [Address; 2] = [
    Address {
        service: "org.freedesktop.UPower.PowerProfiles",
        path: "/org/freedesktop/UPower/PowerProfiles",
        interface: "org.freedesktop.UPower.PowerProfiles",
    },
    Address {
        service: "net.hadess.PowerProfiles",
        path: "/net/hadess/PowerProfiles",
        interface: "net.hadess.PowerProfiles",
    },
];

#[zbus::proxy(
    interface = "org.freedesktop.UPower.PowerProfiles",
    default_service = "org.freedesktop.UPower.PowerProfiles",
    default_path = "/org/freedesktop/UPower/PowerProfiles",
    gen_async = false
)]
pub(crate) trait Profiles {
    #[zbus(property)]
    fn active_profile(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn set_active_profile(&self, profile: &str) -> zbus::Result<()>;
}
