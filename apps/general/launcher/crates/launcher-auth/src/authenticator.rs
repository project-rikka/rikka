use std::time::Duration;

use rikka_net::Client;
use zeroize::Zeroizing;

use crate::device_key::DeviceKey;
use crate::error::{Error, Result};
use crate::microsoft::{DeviceCode, Microsoft};
use crate::services::Services;
use crate::xbox::Xbox;

const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub xuid: String,
    pub expires_at: i64,
    pub(crate) access_token: Zeroizing<String>,
    pub(crate) refresh_token: Zeroizing<String>,
}

impl Session {
    #[must_use]
    pub fn access_token(&self) -> &str {
        &self.access_token
    }

    #[must_use]
    pub fn expires_within(&self, seconds: i64) -> bool {
        self.expires_at - seconds <= jiff::Timestamp::now().as_second()
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Session")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}

pub struct Authenticator {
    microsoft: Microsoft,
    xbox: Xbox,
    services: Services,
}

impl std::fmt::Debug for Authenticator {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Authenticator")
            .field("services", &self.services)
            .finish_non_exhaustive()
    }
}

impl Authenticator {
    pub async fn open(user_agent: &str) -> Result<Authenticator> {
        let http = Client::builder()
            .user_agent(user_agent)
            .timeout(TIMEOUT)
            .build()?;
        let key = DeviceKey::load().await?;
        Ok(Authenticator {
            microsoft: Microsoft::new()?,
            xbox: Xbox::new(http.clone(), key),
            services: Services::new(http),
        })
    }

    #[must_use]
    pub fn services(&self) -> &Services {
        &self.services
    }

    pub async fn begin(&self) -> Result<DeviceCode> {
        self.microsoft.begin().await
    }

    pub async fn complete(&self, code: DeviceCode) -> Result<Session> {
        let token = self.microsoft.complete(code).await?;
        self.session(token, true).await
    }

    pub async fn renew(&self, refresh_token: &str) -> Result<Session> {
        let token = self.microsoft.refresh(refresh_token).await?;
        self.session(token, false).await
    }

    async fn session(&self, token: rikka_oauth::Token, check_license: bool) -> Result<Session> {
        let refresh_token = token
            .refresh_token()
            .map(str::to_owned)
            .ok_or(Error::Expired)?;
        let identity = self.xbox.authorize(token.access_token()).await?;
        let minecraft = self.services.login(&identity).await?;
        if check_license {
            self.services.check_license(&minecraft.access_token).await?;
        }
        let profile = self.services.profile(&minecraft.access_token).await?;
        Ok(Session {
            id: profile.id,
            name: profile.name,
            xuid: identity.xuid,
            expires_at: jiff::Timestamp::now().as_second() + minecraft.expires_in,
            access_token: Zeroizing::new(minecraft.access_token),
            refresh_token: Zeroizing::new(refresh_token),
        })
    }
}
