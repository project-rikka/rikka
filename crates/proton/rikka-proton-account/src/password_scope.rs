use serde::Serialize;

use rikka_proton_pgp::Secret;

use crate::account::Account;
use crate::error::{Error, Result};
use crate::fido2::Fido2Assertion;
use crate::srp;

const PASSWORD_PATH: &str = "core/v4/users/password";

#[must_use]
#[derive(Debug)]
pub struct PasswordScope {
    account: Account,
    username: Option<String>,
    password: Secret,
    totp: Option<String>,
    fido2: Option<Fido2Assertion>,
}

impl PasswordScope {
    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Secret::new(password);
        self
    }

    pub fn totp(mut self, code: impl Into<String>) -> Self {
        self.totp = Some(code.into());
        self
    }

    pub fn fido2_assertion(mut self, assertion: Fido2Assertion) -> Self {
        self.fido2 = Some(assertion);
        self
    }

    pub async fn execute(self) -> Result<()> {
        let username = self.account.resolve_username(self.username).await?;
        let info = self.account.srp_info(&username).await?;
        let proofs = srp::proofs(&info, &username, &self.password).await?;

        let request = PasswordRequest {
            client_ephemeral: proofs.client_ephemeral(),
            client_proof: proofs.client_proof(),
            srp_session: info.srp_session().ok_or(Error::Incomplete("SRPSession"))?,
            totp: self.totp.as_deref(),
            fido2: self.fido2.as_ref(),
        };

        let server_proof: Option<String> = self
            .account
            .api()
            .put(PASSWORD_PATH)
            .json(&request)
            .send_field_or_default("ServerProof")
            .await?;

        srp::verify(&proofs, server_proof.as_deref())
    }

    pub(crate) fn new(account: Account) -> PasswordScope {
        PasswordScope {
            account,
            username: None,
            password: Secret::default(),
            totp: None,
            fido2: None,
        }
    }
}

#[derive(Debug, Serialize)]
struct PasswordRequest<'a> {
    #[serde(rename = "ClientEphemeral")]
    client_ephemeral: &'a str,
    #[serde(rename = "ClientProof")]
    client_proof: &'a str,
    #[serde(rename = "SRPSession")]
    srp_session: &'a str,
    #[serde(rename = "TwoFactorCode", skip_serializing_if = "Option::is_none")]
    totp: Option<&'a str>,
    #[serde(rename = "FIDO2", skip_serializing_if = "Option::is_none")]
    fido2: Option<&'a Fido2Assertion>,
}
