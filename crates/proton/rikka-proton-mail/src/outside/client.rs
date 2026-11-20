use rikka_proton_api::Api;
use rikka_proton_pgp::{Pgp, Secret};

use crate::error::{Error, Result};
use crate::outside::session::OutsideSession;
use crate::outside::{TOKEN, password_decryption_error};

#[derive(Debug, Clone)]
pub struct OutsideClient {
    api: Api,
    pgp: Pgp,
}

impl OutsideClient {
    #[must_use]
    pub fn new(api: Api) -> OutsideClient {
        OutsideClient::with_pgp(api, Pgp::new())
    }

    #[must_use]
    pub fn with_pgp(api: Api, pgp: Pgp) -> OutsideClient {
        OutsideClient { api, pgp }
    }

    #[must_use]
    pub fn api(&self) -> &Api {
        &self.api
    }

    pub async fn unlock(
        &self,
        id: impl Into<String>,
        password: impl Into<Secret>,
    ) -> Result<OutsideSession> {
        let id = id.into();
        let password = password.into();

        if id.is_empty()
            || id.chars().any(|character| {
                character.is_whitespace() || matches!(character, '/' | '?' | '#' | '%')
            })
        {
            return Err(Error::InvalidOutsideLink(id));
        }

        let response: String = self
            .api
            .get(format!("{TOKEN}/{id}"))
            .anonymous()
            .send_field("Token")
            .await?;

        let pgp = self.pgp;
        let encrypted_token = response;
        let token_password = password.clone();
        let token = rikka_proton_api::offload(move || {
            pgp.decrypt()
                .with_password(token_password.expose())
                .message(encrypted_token.as_bytes())
                .map_err(password_decryption_error)
        })
        .await?;

        let token = String::from_utf8(token.into_data()).map_err(|_| Error::WrongPassword)?;

        Ok(OutsideSession::new(
            self.api.clone(),
            self.pgp,
            id,
            Secret::new(token.trim()),
            password,
        ))
    }
}
