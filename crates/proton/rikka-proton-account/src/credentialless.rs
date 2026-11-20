use serde::Serialize;
use serde_json::Value;

use crate::account::Account;
use crate::challenge::Challenge;
use crate::error::Result;
use crate::session_info::SessionInfo;

const CREDENTIALLESS_PATH: &str = "auth/v4/credentialless";

#[must_use]
#[derive(Debug)]
pub struct Credentialless {
    account: Account,
    payload: Option<Value>,
}

impl Credentialless {
    pub fn payload(mut self, payload: Value) -> Self {
        self.payload = Some(payload);
        self
    }

    pub async fn execute(self) -> Result<SessionInfo> {
        self.account.ensure_session().await?;

        let payload = match self.payload {
            Some(payload) => payload,
            None => Challenge::collect(self.account.api())
                .await
                .device_payload(),
        };
        let session: SessionInfo = self
            .account
            .api()
            .post(CREDENTIALLESS_PATH)
            .json(&CredentiallessRequest { payload })
            .send()
            .await?;

        self.account.api().set_session(session.session())?;

        Ok(session)
    }

    pub(crate) fn new(account: Account) -> Credentialless {
        Credentialless {
            account,
            payload: None,
        }
    }
}

#[derive(Debug, Serialize)]
struct CredentiallessRequest {
    #[serde(rename = "Payload")]
    payload: Value,
}
