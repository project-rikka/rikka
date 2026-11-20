use rikka_proton_api::ResponseCode;
use serde::Serialize;

use crate::account::Account;
use crate::auth_info::AuthInfo;
use crate::error::{Error, Result};
use crate::login::LOGIN_PATH;
use crate::session_info::SessionInfo;

#[derive(Debug)]
pub struct Sso {
    account: Account,
    username: String,
    challenge_token: String,
}

impl Sso {
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    #[must_use]
    pub fn challenge_token(&self) -> &str {
        &self.challenge_token
    }

    pub async fn respond(self, response_token: impl Into<String>) -> Result<SessionInfo> {
        let request = SsoRequest {
            response_token: response_token.into(),
        };

        let session = self
            .account
            .api()
            .post(LOGIN_PATH)
            .json(&request)
            .send::<SessionInfo>()
            .await
            .map_err(|error| match error.code() {
                Some(ResponseCode::AuthSwitchToSrp) => Error::PasswordRequired,
                _ => error.into(),
            })?;

        self.account.api().set_session(session.session())?;

        Ok(session)
    }

    pub(crate) fn new(account: Account, username: String, info: &AuthInfo) -> Result<Sso> {
        let challenge_token = info
            .sso_challenge_token()
            .ok_or(Error::Incomplete("SSOChallengeToken"))?
            .to_owned();

        Ok(Sso {
            account,
            username,
            challenge_token,
        })
    }
}

#[derive(Debug, Serialize)]
struct SsoRequest {
    #[serde(rename = "SSOResponseToken")]
    response_token: String,
}
