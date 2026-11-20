use rikka_proton_api::ResponseCode;
use rikka_proton_pgp::Secret;
use serde::Serialize;
use serde_json::Value;

use crate::account::Account;
use crate::auth_info::AuthIntent;
use crate::challenge::Challenge;
use crate::error::{Error, Result};
use crate::field_activity::FieldActivity;
use crate::outcome::{self, LoginOutcome};
use crate::session_info::SessionInfo;
use crate::srp;
use crate::sso::Sso;
use crate::two_factor::TwoFactor;

pub(crate) const LOGIN_PATH: &str = "auth/v4";

#[must_use]
#[derive(Debug)]
pub struct Login {
    account: Account,
    username: Option<String>,
    password: Secret,
    intent: AuthIntent,
    payload: Option<Value>,
    username_activity: Option<FieldActivity>,
}

impl Login {
    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Secret::new(password);
        self
    }

    pub fn intent(mut self, intent: AuthIntent) -> Self {
        self.intent = intent;
        self
    }

    pub fn payload(mut self, payload: Value) -> Self {
        self.payload = Some(payload);
        self
    }

    pub fn username_activity(mut self, activity: FieldActivity) -> Self {
        self.username_activity = Some(activity);
        self
    }

    pub async fn execute(self) -> Result<LoginOutcome> {
        let username = self.username.ok_or(Error::MissingUsername)?;
        let info = self.account.auth_info(&username, self.intent).await?;

        if info.is_sso() {
            return Ok(LoginOutcome::Sso(Box::new(Sso::new(
                self.account,
                username,
                &info,
            )?)));
        }

        let proofs = srp::proofs(&info, &username, &self.password).await?;
        let payload = match self.payload {
            Some(payload) => payload,
            None => Challenge::collect(self.account.api())
                .await
                .login_payload(self.username_activity.as_ref()),
        };
        let request = LoginRequest {
            username: &username,
            client_ephemeral: proofs.client_ephemeral(),
            client_proof: proofs.client_proof(),
            srp_session: info.srp_session().ok_or(Error::Incomplete("SRPSession"))?,
            payload: &payload,
        };

        let response = self
            .account
            .api()
            .post(LOGIN_PATH)
            .json(&request)
            .send::<SessionInfo>()
            .await;

        let session = match response {
            Ok(session) => session,
            Err(error) => return handle_login_error(self.account, username, error).await,
        };

        srp::verify(&proofs, session.server_proof())?;
        self.account.api().set_session(session.session())?;

        if session.needs_second_factor() {
            return Ok(LoginOutcome::TwoFactorRequired(Box::new(TwoFactor::new(
                self.account,
                session,
                self.password,
            ))));
        }

        outcome::complete_login(self.account, session, self.password).await
    }

    pub(crate) fn new(account: Account) -> Login {
        Login {
            account,
            username: None,
            password: Secret::default(),
            intent: AuthIntent::default(),
            payload: None,
            username_activity: None,
        }
    }
}

async fn handle_login_error(
    account: Account,
    username: String,
    error: rikka_proton_api::Error,
) -> Result<LoginOutcome> {
    match error.code() {
        Some(ResponseCode::AuthSwitchToSso) => {
            let info = account.auth_info(&username, AuthIntent::Sso).await?;
            Ok(LoginOutcome::Sso(Box::new(Sso::new(
                account, username, &info,
            )?)))
        }
        Some(ResponseCode::PasswordWrong) => Err(Error::WrongPassword),
        _ => Err(error.into()),
    }
}

#[derive(Debug, Serialize)]
struct LoginRequest<'a> {
    #[serde(rename = "Username")]
    username: &'a str,
    #[serde(rename = "ClientEphemeral")]
    client_ephemeral: &'a str,
    #[serde(rename = "ClientProof")]
    client_proof: &'a str,
    #[serde(rename = "SRPSession")]
    srp_session: &'a str,
    #[serde(rename = "Payload")]
    payload: &'a Value,
}
