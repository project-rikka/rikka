use rikka_oauth::{Client, DeviceAuthorization, Token};

use crate::error::{Error, Result};

pub(crate) const CLIENT_ID: &str = "00000000402b5328";
const SCOPE: &str = "service::user.auth.xboxlive.com::MBI_SSL";
const DEVICE_ENDPOINT: &str = "https://login.live.com/oauth20_connect.srf";
const TOKEN_ENDPOINT: &str = "https://login.live.com/oauth20_token.srf";

pub struct DeviceCode(DeviceAuthorization);

impl DeviceCode {
    #[must_use]
    pub fn user_code(&self) -> &str {
        self.0.user_code()
    }

    #[must_use]
    pub fn verification_url(&self) -> &str {
        self.0.verification_url()
    }
}

impl std::fmt::Debug for DeviceCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeviceCode")
            .field("user_code", &self.user_code())
            .finish_non_exhaustive()
    }
}

pub(crate) struct Microsoft {
    client: Client,
}

impl Microsoft {
    pub(crate) fn new() -> Result<Microsoft> {
        Ok(Microsoft {
            client: Client::new(CLIENT_ID, TOKEN_ENDPOINT)?,
        })
    }

    pub(crate) async fn begin(&self) -> Result<DeviceCode> {
        let authorization = self
            .client
            .device(DEVICE_ENDPOINT)
            .scope(SCOPE)
            .field("response_type", "device_code")
            .send()
            .await?;
        Ok(DeviceCode(authorization))
    }

    pub(crate) async fn complete(&self, code: DeviceCode) -> Result<Token> {
        Ok(code.0.token().await?)
    }

    pub(crate) async fn refresh(&self, refresh_token: &str) -> Result<Token> {
        match self
            .client
            .refresh(refresh_token)
            .field("scope", SCOPE)
            .send()
            .await
        {
            Err(rikka_oauth::Error::ServerResponse(response)) if response.is_expired() => {
                Err(Error::Expired)
            }
            refreshed => Ok(refreshed?),
        }
    }
}
