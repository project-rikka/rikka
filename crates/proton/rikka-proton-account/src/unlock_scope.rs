use serde::Serialize;

use rikka_proton_pgp::Secret;

use crate::account::Account;
use crate::error::{Error, Result};
use crate::srp;

const UNLOCK_PATH: &str = "core/v4/users/unlock";

#[must_use]
#[derive(Debug)]
pub struct UnlockScope {
    account: Account,
    username: Option<String>,
    password: Secret,
}

impl UnlockScope {
    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Secret::new(password);
        self
    }

    pub async fn execute(self) -> Result<()> {
        let username = self.account.resolve_username(self.username).await?;
        let info = self.account.srp_info(&username).await?;
        let proofs = srp::proofs(&info, &username, &self.password).await?;

        let request = UnlockRequest {
            client_ephemeral: proofs.client_ephemeral(),
            client_proof: proofs.client_proof(),
            srp_session: info.srp_session().ok_or(Error::Incomplete("SRPSession"))?,
        };

        let server_proof: Option<String> = self
            .account
            .api()
            .put(UNLOCK_PATH)
            .json(&request)
            .send_field_or_default("ServerProof")
            .await?;

        srp::verify(&proofs, server_proof.as_deref())
    }

    pub(crate) fn new(account: Account) -> UnlockScope {
        UnlockScope {
            account,
            username: None,
            password: Secret::default(),
        }
    }
}

#[derive(Debug, Serialize)]
struct UnlockRequest<'a> {
    #[serde(rename = "ClientEphemeral")]
    client_ephemeral: &'a str,
    #[serde(rename = "ClientProof")]
    client_proof: &'a str,
    #[serde(rename = "SRPSession")]
    srp_session: &'a str,
}
