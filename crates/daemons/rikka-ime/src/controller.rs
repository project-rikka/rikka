pub(crate) type Entry = (String, String);

pub(crate) type Available = (String, String, String, String, String, String, bool);

pub(crate) const INTERFACE: &str = "org.fcitx.Fcitx.Controller1";
pub(crate) const PATH: &str = "/controller";

#[zbus::proxy(
    interface = "org.fcitx.Fcitx.Controller1",
    default_service = "org.fcitx.Fcitx5",
    default_path = "/controller"
)]
pub(crate) trait Controller {
    fn available_input_methods(&self) -> zbus::Result<Vec<Available>>;

    fn current_input_method_group(&self) -> zbus::Result<String>;

    fn input_method_group_info(&self, group: &str) -> zbus::Result<(String, Vec<Entry>)>;

    fn set_input_method_group_info(
        &self,
        group: &str,
        default_layout: &str,
        entries: &[Entry],
    ) -> zbus::Result<()>;

    #[zbus(name = "ConfigureIM")]
    fn configure_input_method(&self, engine: &str) -> zbus::Result<()>;
}
