use std::sync::Arc;

use rikka_proton_account::{AccountKeys, ProfileRefresh};
use rikka_proton_api::offload;

use crate::client::Proton;
use crate::error::Result;
use crate::event_loop::EventLoop;
use crate::renewal::Renewal;

#[derive(Debug, Clone)]
pub struct Session {
    proton: Proton,
    keys: Arc<AccountKeys>,
}

impl Session {
    pub(crate) fn new(proton: Proton, keys: AccountKeys) -> Session {
        Session {
            proton,
            keys: Arc::new(keys),
        }
    }

    #[must_use]
    pub fn proton(&self) -> &Proton {
        &self.proton
    }

    #[must_use]
    pub fn keys(&self) -> &Arc<AccountKeys> {
        &self.keys
    }

    pub fn event_loop<C: Send + 'static>(&self) -> EventLoop<C> {
        EventLoop::new(self.clone())
    }

    pub async fn renew(&self) -> Result<Renewal> {
        Ok(match self.proton.account().refresh(&self.keys).await? {
            ProfileRefresh::Unchanged => Renewal::Unchanged,
            ProfileRefresh::Updated(keys) => {
                Renewal::Updated(Box::new(Session::new(self.proton.clone(), *keys)))
            }
            ProfileRefresh::Rekeyed(keys) => {
                Renewal::Rekeyed(Box::new(Session::new(self.proton.clone(), *keys)))
            }
        })
    }

    pub async fn sign_out(&self) -> Result<()> {
        let revoked = self.proton.sign_out().await;
        if let Some(cache) = self.proton.account().cache().cloned() {
            offload(move || cache.retire()).await?;
        }
        revoked
    }
}
