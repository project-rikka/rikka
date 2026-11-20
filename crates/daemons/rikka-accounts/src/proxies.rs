use zbus::zvariant::OwnedObjectPath;

#[zbus::proxy(
    interface = "org.freedesktop.Accounts",
    default_service = "org.freedesktop.Accounts",
    default_path = "/org/freedesktop/Accounts",
    gen_blocking = false
)]
pub(crate) trait Manager {
    fn find_user_by_id(&self, id: i64) -> zbus::Result<OwnedObjectPath>;
}

#[zbus::proxy(
    interface = "org.freedesktop.Accounts.User",
    default_service = "org.freedesktop.Accounts",
    gen_blocking = false
)]
pub(crate) trait User {
    fn set_icon_file(&self, filename: &str) -> zbus::Result<()>;

    #[zbus(property(emits_changed_signal = "false"))]
    fn account_type(&self) -> zbus::Result<i32>;

    #[zbus(property(emits_changed_signal = "false"))]
    fn icon_file(&self) -> zbus::Result<String>;

    #[zbus(signal)]
    fn changed(&self) -> zbus::Result<()>;
}
