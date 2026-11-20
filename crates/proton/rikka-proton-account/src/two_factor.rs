use rikka_fido::{Assertion, AssertionRequest};
use rikka_proton_pgp::Secret;
use serde::{Deserialize, Serialize};

use crate::account::Account;
use crate::error::{Error, Result};
use crate::fido2::{AuthenticationOptions, Fido2Assertion, Fido2Info};
use crate::outcome::{self, LoginOutcome};
use crate::second_factor::SecondFactorInfo;
use crate::session_info::SessionInfo;

const SECOND_FACTOR_PATH: &str = "auth/v4/2fa";

#[derive(Debug)]
pub struct TwoFactor {
    account: Account,
    session: SessionInfo,
    password: Secret,
}

impl TwoFactor {
    #[must_use]
    pub fn session(&self) -> &SessionInfo {
        &self.session
    }

    #[must_use]
    pub fn info(&self) -> Option<&SecondFactorInfo> {
        self.session.second_factor()
    }

    #[must_use]
    pub fn has_totp(&self) -> bool {
        self.info().is_some_and(SecondFactorInfo::has_totp)
    }

    #[must_use]
    pub fn has_fido2(&self) -> bool {
        self.info().is_some_and(SecondFactorInfo::has_fido2)
    }

    #[must_use]
    pub fn fido2(&self) -> Option<&Fido2Info> {
        self.info().and_then(SecondFactorInfo::fido2)
    }

    #[must_use]
    pub fn security_key_request(&self) -> Option<AssertionRequest> {
        let options = self.authentication_options()?;
        Some(options.assertion_request(options.web_origin()))
    }

    pub async fn security_key(self, assertion: &Assertion) -> Result<LoginOutcome> {
        let options = self
            .authentication_options()
            .cloned()
            .ok_or(Error::Incomplete("FIDO2 authentication options"))?;
        self.fido2_assertion(Fido2Assertion::from_assertion(options, assertion))
            .await
    }

    pub async fn totp(self, code: impl Into<String>) -> Result<LoginOutcome> {
        self.submit(SecondFactorRequest {
            totp: Some(code.into()),
            fido2: None,
        })
        .await
    }

    pub async fn fido2_assertion(self, assertion: Fido2Assertion) -> Result<LoginOutcome> {
        self.submit(SecondFactorRequest {
            totp: None,
            fido2: Some(assertion),
        })
        .await
    }

    fn authentication_options(&self) -> Option<&AuthenticationOptions> {
        self.fido2().and_then(Fido2Info::authentication_options)
    }

    pub(crate) fn new(account: Account, session: SessionInfo, password: Secret) -> TwoFactor {
        TwoFactor {
            account,
            session,
            password,
        }
    }

    async fn submit(mut self, request: SecondFactorRequest) -> Result<LoginOutcome> {
        let response: ScopesResponse = self
            .account
            .api()
            .post(SECOND_FACTOR_PATH)
            .json(&request)
            .send()
            .await?;

        self.session.set_scopes(response.scopes);
        self.session.clear_second_factor();
        self.account.api().set_session(self.session.session())?;

        outcome::complete_login(self.account, self.session, self.password).await
    }
}

#[derive(Debug, Serialize)]
struct SecondFactorRequest {
    #[serde(rename = "TwoFactorCode", skip_serializing_if = "Option::is_none")]
    totp: Option<String>,
    #[serde(rename = "FIDO2", skip_serializing_if = "Option::is_none")]
    fido2: Option<Fido2Assertion>,
}

#[derive(Debug, Deserialize)]
struct ScopesResponse {
    #[serde(
        rename = "Scopes",
        default,
        deserialize_with = "rikka_proton_api::scopes::deserialize"
    )]
    scopes: Vec<String>,
}
