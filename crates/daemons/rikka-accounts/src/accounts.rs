use std::path::{Path, PathBuf};

use futures_util::{Stream, StreamExt};
use rikka_bus::DaemonError;
use zbus::proxy::CacheProperties;

use crate::account::{Account, AccountKind};
use crate::error::{Error, Result};
use crate::proxies::{ManagerProxy, UserProxy};

const NO_ICON: &str = "";

#[derive(Clone, Debug)]
pub struct Accounts {
    user: UserProxy<'static>,
}

impl Accounts {
    pub async fn current() -> Result<Accounts> {
        let connection = zbus::Connection::system()
            .await
            .map_err(Error::unreachable)?;
        let manager = ManagerProxy::new(&connection).await?;
        let path = manager
            .find_user_by_id(i64::from(rikka_dirs::passwd::current_uid()))
            .await?;
        let user = UserProxy::builder(&connection)
            .path(path)?
            .cache_properties(CacheProperties::No)
            .build()
            .await?;
        Ok(Accounts { user })
    }

    pub async fn account(&self) -> Result<Account> {
        let (kind, icon) =
            futures_util::try_join!(self.user.account_type(), self.user.icon_file())?;
        Ok(Account {
            kind: AccountKind::from_code(kind),
            icon: (!icon.is_empty()).then(|| PathBuf::from(icon)),
        })
    }

    pub async fn set_icon(&self, file: &Path) -> Result<()> {
        let file = file
            .to_str()
            .ok_or_else(|| Error::refused(format!("{} is not UTF-8", file.display())))?;
        Ok(self.user.set_icon_file(file).await?)
    }

    pub async fn clear_icon(&self) -> Result<()> {
        Ok(self.user.set_icon_file(NO_ICON).await?)
    }

    pub async fn changes(&self) -> Result<impl Stream<Item = ()> + Send + 'static> {
        Ok(self.user.receive_changed().await?.map(|_| ()))
    }
}
