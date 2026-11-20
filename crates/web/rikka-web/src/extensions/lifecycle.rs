use rikka_gecko_protocol::geckoview::webextension::extension::Extension;
use rikka_gecko_protocol::geckoview::webextension::lifecycle::OnInstallationFailed;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lifecycle {
    Installing,
    Installed,
    Uninstalling,
    Uninstalled,
    Enabling,
    Enabled,
    Disabling,
    Disabled,
    Ready,
    PermissionsChanged,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExtensionChange {
    pub lifecycle: Lifecycle,
    pub extension: Extension,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InstallFailure {
    pub extension: Option<Extension>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub error: i32,
}

impl InstallFailure {
    pub(crate) fn from_wire(failed: OnInstallationFailed) -> InstallFailure {
        InstallFailure {
            extension: failed.extension,
            id: failed.addon_id,
            name: failed.addon_name,
            version: failed.addon_version,
            error: failed.error,
        }
    }
}
