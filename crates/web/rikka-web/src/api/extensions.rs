use std::future::Future;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::json;

use crate::channel::Link;
use crate::error::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallKind {
    Admin,
    Development,
    Normal,
    Sideload,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionKind {
    Extension,
    HostedApp,
    PackagedApp,
    LegacyPackagedApp,
    Theme,
    LoginScreenExtension,
    #[serde(other)]
    Other,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub may_disable: bool,
    pub install_type: InstallKind,
    pub options_url: Option<String>,
    pub homepage_url: Option<String>,
    #[serde(rename = "type")]
    pub kind: ExtensionKind,
}

impl ExtensionInfo {
    #[must_use]
    pub fn settings_page(&self) -> String {
        format!("chrome://extensions/?id={}", self.id)
    }
}

#[derive(Clone, Debug)]
pub struct Extensions {
    link: Arc<Link>,
}

impl Extensions {
    pub(crate) fn new(link: Arc<Link>) -> Extensions {
        Extensions { link }
    }

    pub fn installed(&self) -> impl Future<Output = Result<Vec<ExtensionInfo>>> + Send + use<> {
        self.link.call("management.getAll", Vec::new()).decoded()
    }

    pub fn set_enabled(
        &self,
        id: &str,
        enabled: bool,
    ) -> impl Future<Output = Result<()>> + Send + use<> {
        self.link
            .call("management.setEnabled", vec![json!(id), json!(enabled)])
            .done()
    }

    pub fn uninstall(&self, id: &str) -> impl Future<Output = Result<()>> + Send + use<> {
        self.link
            .call(
                "management.uninstall",
                vec![json!(id), json!({ "showConfirmDialog": true })],
            )
            .done()
    }
}
