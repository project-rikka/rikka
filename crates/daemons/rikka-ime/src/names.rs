use rikka_bus::Rule;

pub(crate) const BUS: &str = "org.freedesktop.DBus";

pub(crate) const OWNER_CHANGED: &str = "NameOwnerChanged";

pub(crate) const PANEL: &str = "org.kde.impanel";

pub(crate) const PANEL_PATH: &str = "/org/kde/impanel";

pub(crate) const PANEL_V1: &str = "org.kde.impanel";

pub(crate) const PANEL_V2: &str = "org.kde.impanel2";

pub(crate) const INPUT_METHOD: &str = "org.kde.kimpanel.inputmethod";

pub(crate) const FCITX: &str = "org.fcitx.Fcitx5";

pub(crate) const INPUT_METHOD_PATH: &str = "/kimpanel";

pub(crate) const CURRENT_INPUT_METHOD: &str = "/Fcitx/im";

pub(crate) const INPUT_METHOD_ENTRY: &str = "/Fcitx/im/";

pub(crate) const TRIGGER_PROPERTY: &str = "TriggerProperty";

pub(crate) fn owner_changed() -> Rule {
    Rule::new()
        .interface(BUS)
        .member(OWNER_CHANGED)
        .arg(0, FCITX)
}
