use zbus::zvariant::OwnedObjectPath;

#[zbus::proxy(
    interface = "org.freedesktop.GeoClue2.Manager",
    default_service = "org.freedesktop.GeoClue2",
    default_path = "/org/freedesktop/GeoClue2/Manager",
    gen_async = false
)]
pub(crate) trait Manager {
    fn get_client(&self) -> zbus::Result<OwnedObjectPath>;
}

#[zbus::proxy(
    interface = "org.freedesktop.GeoClue2.Client",
    default_service = "org.freedesktop.GeoClue2",
    gen_async = false
)]
pub(crate) trait Client {
    fn start(&self) -> zbus::Result<()>;

    fn stop(&self) -> zbus::Result<()>;

    #[zbus(property)]
    fn location(&self) -> zbus::Result<OwnedObjectPath>;

    #[zbus(property)]
    fn set_desktop_id(&self, id: &str) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_requested_accuracy_level(&self, level: u32) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_distance_threshold(&self, meters: u32) -> zbus::Result<()>;
}
