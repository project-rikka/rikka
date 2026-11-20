use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rikka_gecko_protocol::Target;
use rikka_gecko_protocol::geckoview::permission::{
    ContentPermission, GetAllPermissions, GetPermissionsByUri, PermissionValue, SetPermission,
    SetPermissionByUri,
};
use rikka_gecko_protocol::geckoview::session::SessionContextId;
use rikka_gecko_protocol::geckoview::storage::{
    ClearBaseDomainData, ClearData, ClearFlags, ClearHostData,
};
use rikka_gecko_protocol::rikka::storage::{
    ClearDataBetween, Cookie, GetCookies, GetSiteData, SiteData,
};

use crate::error::{Error, Result};
use crate::wire::Link;

#[derive(Clone, Debug)]
pub struct Storage {
    link: Arc<Link>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Partition {
    pub context: Option<String>,
    pub private: bool,
}

impl Partition {
    fn context_id(&self) -> Option<SessionContextId> {
        self.context.as_deref().map(SessionContextId::from_unsafe)
    }

    fn private_id(&self) -> u32 {
        u32::from(self.private)
    }
}

impl Storage {
    pub(crate) fn new(link: &Arc<Link>) -> Storage {
        Storage {
            link: Arc::clone(link),
        }
    }

    pub async fn clear(&self, kinds: ClearFlags) -> Result<()> {
        self.link
            .ask(Target::Runtime, &ClearData { flags: kinds })
            .await
    }

    pub async fn clear_host(&self, host: impl Into<String>, kinds: ClearFlags) -> Result<()> {
        let clear = ClearHostData {
            host: host.into(),
            flags: kinds,
        };
        self.link.ask(Target::Runtime, &clear).await
    }

    pub async fn clear_base_domain(
        &self,
        domain: impl Into<String>,
        kinds: ClearFlags,
    ) -> Result<()> {
        let clear = ClearBaseDomainData {
            base_domain: domain.into(),
            flags: kinds,
        };
        self.link.ask(Target::Runtime, &clear).await
    }

    pub async fn clear_between(
        &self,
        from: SystemTime,
        to: SystemTime,
        kinds: ClearFlags,
    ) -> Result<()> {
        let clear = ClearDataBetween {
            from: millis(from)?,
            to: millis(to)?,
            flags: kinds,
        };
        self.link.ask(Target::Runtime, &clear).await
    }

    pub async fn site_data(
        &self,
        host: impl Into<String>,
        partition: &Partition,
    ) -> Result<SiteData> {
        let query = GetSiteData {
            host: host.into(),
            private: partition.private,
            context_id: partition.context_id(),
        };
        self.link.ask(Target::Runtime, &query).await
    }

    pub async fn cookies(
        &self,
        uri: impl Into<String>,
        partition: &Partition,
    ) -> Result<Vec<Cookie>> {
        let query = GetCookies {
            uri: uri.into(),
            private: partition.private,
            context_id: partition.context_id(),
        };
        self.link.ask(Target::Runtime, &query).await
    }

    pub async fn permissions(&self) -> Result<Vec<ContentPermission>> {
        let list = self.link.ask(Target::Runtime, &GetAllPermissions).await?;
        Ok(list.permissions)
    }

    pub async fn permissions_for(
        &self,
        uri: impl Into<String>,
        partition: &Partition,
    ) -> Result<Vec<ContentPermission>> {
        let query = GetPermissionsByUri {
            uri: uri.into(),
            context_id: partition.context_id(),
            private_browsing_id: partition.private_id(),
        };
        let list = self.link.ask(Target::Runtime, &query).await?;
        Ok(list.permissions)
    }

    pub fn set_permission(
        &self,
        permission: &ContentPermission,
        value: PermissionValue,
        keep_in_private: bool,
    ) {
        self.link.send(
            Target::Runtime,
            &SetPermission {
                permission: permission.clone(),
                new_value: value,
                allow_permanent_private_browsing: keep_in_private,
            },
        );
    }

    pub fn set_permission_for(
        &self,
        uri: impl Into<String>,
        permission: impl Into<String>,
        value: PermissionValue,
        partition: &Partition,
    ) {
        self.link.send(
            Target::Runtime,
            &SetPermissionByUri {
                uri: uri.into(),
                context_id: partition.context_id(),
                private_id: partition.private_id(),
                perm: permission.into(),
                new_value: value,
            },
        );
    }
}

fn millis(time: SystemTime) -> Result<u64> {
    let since = time
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::BeforeEpoch)?;
    Ok(u64::try_from(since.as_millis()).unwrap_or(u64::MAX))
}
