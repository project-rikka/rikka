use std::sync::Arc;

use rikka_net::{RequestBuilder, Response, StatusCode};
use tokio::sync::Mutex;

use crate::endpoint::Endpoint;
use crate::error::Result;
use crate::master::{Master, now};
use crate::secret::Secret;
use crate::service::Service;

const RENEWAL_MARGIN: u64 = 300;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessToken {
    token: Arc<Secret>,
    expires_at: u64,
}

#[derive(Debug)]
pub struct Grants {
    endpoint: Endpoint,
    master: Master,
    slots: [Mutex<Option<AccessToken>>; Service::ALL.len()],
}

impl AccessToken {
    pub(crate) fn new(token: &str, expires_at: u64) -> AccessToken {
        AccessToken {
            token: Arc::new(Secret::new(token)),
            expires_at,
        }
    }

    #[must_use]
    pub fn expose(&self) -> &str {
        self.token.expose()
    }

    fn is_fresh(&self) -> bool {
        now() + RENEWAL_MARGIN < self.expires_at
    }
}

impl Grants {
    #[must_use]
    pub fn new(endpoint: Endpoint, master: Master) -> Grants {
        Grants {
            endpoint,
            master,
            slots: Default::default(),
        }
    }

    #[must_use]
    pub fn master(&self) -> &Master {
        &self.master
    }

    pub async fn token(&self, service: Service) -> Result<AccessToken> {
        let mut slot = self.slots[service.slot()].lock().await;
        if let Some(token) = slot.as_ref().filter(|token| token.is_fresh()) {
            return Ok(token.clone());
        }
        let token = self.master.mint(&self.endpoint, service).await?;
        *slot = Some(token.clone());
        Ok(token)
    }

    pub async fn renew(&self, service: Service, spent: &AccessToken) -> Result<AccessToken> {
        let mut slot = self.slots[service.slot()].lock().await;
        if let Some(token) = slot.as_ref().filter(|token| *token != spent) {
            return Ok(token.clone());
        }
        let token = self.master.mint(&self.endpoint, service).await?;
        *slot = Some(token.clone());
        Ok(token)
    }

    pub async fn send(
        &self,
        service: Service,
        request: impl Fn(&str) -> RequestBuilder,
    ) -> Result<Response> {
        let token = self.token(service).await?;
        let response = request(token.expose()).send().await?;
        if response.status() != StatusCode::UNAUTHORIZED {
            return Ok(response);
        }
        let token = self.renew(service, &token).await?;
        Ok(request(token.expose()).send().await?)
    }
}
