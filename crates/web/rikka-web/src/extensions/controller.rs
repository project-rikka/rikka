use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use base64::Engine;
use rikka_gecko_protocol::Target;
use rikka_gecko_protocol::geckoview::webextension::action::{
    ActionDelegateAttached, BrowserActionClick, PageActionClick,
};
use rikka_gecko_protocol::geckoview::webextension::controller::{
    AddOptionalPermissions, Disable, Enable, EnableSource, Get, Install, List,
    RemoveOptionalPermissions, SetPrivateBrowsingAllowed, Uninstall, Update,
};
use rikka_gecko_protocol::geckoview::webextension::extension::{Extension, PermissionSet};
use rikka_gecko_protocol::rikka::extensions::GetExtensionIcon;

use crate::error::{Error, Result};
use crate::uris;
use crate::wire::Link;

static INSTALLS: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionIcon {
    pub mime_type: Option<String>,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct Extensions {
    link: Arc<Link>,
}

impl Extensions {
    pub(crate) fn new(link: &Arc<Link>) -> Extensions {
        Extensions {
            link: Arc::clone(link),
        }
    }

    pub async fn icon(&self, uri: impl Into<String>) -> Result<ExtensionIcon> {
        let icon = self
            .link
            .ask(Target::Runtime, &GetExtensionIcon { uri: uri.into() })
            .await?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(icon.data)
            .map_err(|error| Error::Malformed(error.to_string()))?;
        Ok(ExtensionIcon {
            mime_type: icon.mime_type,
            bytes,
        })
    }

    pub async fn install(&self, uri: impl Into<String>) -> Result<Extension> {
        let install = Install {
            location_uri: uri.into(),
            install_id: format!("rikka-{}", INSTALLS.fetch_add(1, Ordering::Relaxed)),
            install_method: None,
        };
        let installed = self.link.ask(Target::Runtime, &install).await?;
        attach_actions(&self.link, &installed.extension.web_extension_id);
        Ok(installed.extension)
    }

    pub async fn install_file(&self, path: impl AsRef<Path>) -> Result<Extension> {
        let path = path.as_ref();
        if !path.is_absolute() {
            return Err(Error::RelativePath(path.to_owned()));
        }
        self.install(uris::file(path)).await
    }

    pub async fn list(&self) -> Result<Vec<Extension>> {
        let list = self.link.ask(Target::Runtime, &List).await?;
        for extension in &list.extensions {
            attach_actions(&self.link, &extension.web_extension_id);
        }
        Ok(list.extensions)
    }

    pub async fn get(&self, id: impl Into<String>) -> Result<Extension> {
        let get = Get {
            extension_id: id.into(),
        };
        Ok(self.link.ask(Target::Runtime, &get).await?.extension)
    }

    pub async fn enable(&self, id: impl Into<String>) -> Result<Extension> {
        let enable = Enable {
            web_extension_id: id.into(),
            source: EnableSource::User,
        };
        Ok(self.link.ask(Target::Runtime, &enable).await?.extension)
    }

    pub async fn disable(&self, id: impl Into<String>) -> Result<Extension> {
        let disable = Disable {
            web_extension_id: id.into(),
            source: EnableSource::User,
        };
        Ok(self.link.ask(Target::Runtime, &disable).await?.extension)
    }

    pub async fn uninstall(&self, id: impl Into<String>) -> Result<()> {
        let uninstall = Uninstall {
            web_extension_id: id.into(),
        };
        self.link.ask(Target::Runtime, &uninstall).await
    }

    pub async fn update(&self, id: impl Into<String>) -> Result<Option<Extension>> {
        let update = Update {
            web_extension_id: id.into(),
        };
        let updated = self.link.ask(Target::Runtime, &update).await?;
        Ok(updated.map(|reply| reply.extension))
    }

    pub async fn allow_in_private(
        &self,
        id: impl Into<String>,
        allowed: bool,
    ) -> Result<Extension> {
        let allow = SetPrivateBrowsingAllowed {
            extension_id: id.into(),
            allowed,
        };
        Ok(self.link.ask(Target::Runtime, &allow).await?.extension)
    }

    pub async fn grant_optional(
        &self,
        id: impl Into<String>,
        permissions: PermissionSet,
    ) -> Result<Extension> {
        let grant = AddOptionalPermissions {
            extension_id: id.into(),
            permissions: permissions.permissions,
            origins: permissions.origins,
            data_collection_permissions: permissions.data_collection,
        };
        Ok(self.link.ask(Target::Runtime, &grant).await?.extension)
    }

    pub async fn revoke_optional(
        &self,
        id: impl Into<String>,
        permissions: PermissionSet,
    ) -> Result<Extension> {
        let revoke = RemoveOptionalPermissions {
            extension_id: id.into(),
            permissions: permissions.permissions,
            origins: permissions.origins,
            data_collection_permissions: permissions.data_collection,
        };
        Ok(self.link.ask(Target::Runtime, &revoke).await?.extension)
    }

    pub async fn click_browser_action(&self, id: impl Into<String>) -> Result<Option<String>> {
        let click = BrowserActionClick {
            extension_id: id.into(),
        };
        self.link.ask(Target::Runtime, &click).await
    }

    pub async fn click_page_action(&self, id: impl Into<String>) -> Result<Option<String>> {
        let click = PageActionClick {
            extension_id: id.into(),
        };
        self.link.ask(Target::Runtime, &click).await
    }
}

pub(crate) fn attach_actions(link: &Link, id: &str) {
    link.send(
        Target::Runtime,
        &ActionDelegateAttached {
            extension_id: id.to_owned(),
        },
    );
}
