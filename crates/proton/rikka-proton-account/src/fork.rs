use serde::Serialize;

use crate::account::Account;
use crate::error::Result;
use crate::fork_key::ForkKey;
use crate::fork_payload::ForkPayload;

pub(crate) const FORKS_PATH: &str = "auth/v4/sessions/forks";

#[derive(Debug)]
enum RequestPayload {
    Encoded(String),
    Sealed(ForkPayload, ForkKey),
}

#[must_use]
#[derive(Debug)]
pub struct SessionFork {
    account: Account,
    child_client_id: String,
    payload: Option<RequestPayload>,
    independent: bool,
    user_code: Option<String>,
}

impl SessionFork {
    pub fn child_client_id(mut self, child_client_id: impl Into<String>) -> Self {
        self.child_client_id = child_client_id.into();
        self
    }

    pub fn payload(mut self, payload: impl Into<String>) -> Self {
        self.payload = Some(RequestPayload::Encoded(payload.into()));
        self
    }

    pub fn sealed_payload(mut self, payload: ForkPayload, key: ForkKey) -> Self {
        self.payload = Some(RequestPayload::Sealed(payload, key));
        self
    }

    pub fn independent(mut self, independent: bool) -> Self {
        self.independent = independent;
        self
    }

    pub fn user_code(mut self, user_code: impl Into<String>) -> Self {
        self.user_code = Some(user_code.into());
        self
    }

    pub async fn execute(self) -> Result<String> {
        let payload = match self.payload {
            Some(RequestPayload::Sealed(payload, key)) => Some(payload.seal(&key)?),
            Some(RequestPayload::Encoded(payload)) => Some(payload),
            None => None,
        };

        let request = ForkRequest {
            payload,
            child_client_id: self.child_client_id,
            independent: i64::from(self.independent),
            user_code: self.user_code,
        };

        Ok(self
            .account
            .api()
            .post(FORKS_PATH)
            .json(&request)
            .send_field("Selector")
            .await?)
    }

    pub(crate) fn new(account: Account) -> SessionFork {
        SessionFork {
            account,
            child_client_id: String::new(),
            payload: None,
            independent: false,
            user_code: None,
        }
    }
}

#[derive(Debug, Serialize)]
struct ForkRequest {
    #[serde(rename = "Payload")]
    payload: Option<String>,
    #[serde(rename = "ChildClientID")]
    child_client_id: String,
    #[serde(rename = "Independent")]
    independent: i64,
    #[serde(rename = "UserCode", skip_serializing_if = "Option::is_none")]
    user_code: Option<String>,
}
