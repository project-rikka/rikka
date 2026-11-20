use rikka_proton_pgp::SrpChallenge;
use serde::{Deserialize, Serialize};

use crate::second_factor::SecondFactorInfo;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthIntent {
    #[default]
    #[serde(rename = "Auto")]
    Auto,
    #[serde(rename = "Proton")]
    Proton,
    #[serde(rename = "SSO")]
    Sso,
}

impl AuthIntent {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            AuthIntent::Auto => "Auto",
            AuthIntent::Proton => "Proton",
            AuthIntent::Sso => "SSO",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthInfo {
    #[serde(flatten)]
    challenge: SrpChallenge,
    #[serde(rename = "2FA", default, skip_serializing_if = "Option::is_none")]
    second_factor: Option<SecondFactorInfo>,
    #[serde(
        rename = "SSOChallengeToken",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    sso_challenge_token: Option<String>,
}

impl AuthInfo {
    #[must_use]
    pub fn challenge(&self) -> &SrpChallenge {
        &self.challenge
    }

    #[must_use]
    pub fn srp_session(&self) -> Option<&str> {
        self.challenge.session()
    }

    #[must_use]
    pub fn second_factor(&self) -> Option<&SecondFactorInfo> {
        self.second_factor.as_ref()
    }

    #[must_use]
    pub fn sso_challenge_token(&self) -> Option<&str> {
        self.sso_challenge_token.as_deref()
    }

    #[must_use]
    pub fn is_sso(&self) -> bool {
        self.sso_challenge_token.is_some()
    }
}
