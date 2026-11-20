use std::fmt;

use serde::Deserialize;

use crate::mask::Masked;

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct HumanVerificationDetails {
    #[serde(rename = "HumanVerificationMethods")]
    methods: Vec<String>,
    #[serde(rename = "HumanVerificationToken")]
    token: String,
}

impl HumanVerificationDetails {
    #[must_use]
    pub fn methods(&self) -> &[String] {
        &self.methods
    }

    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }
}

impl fmt::Debug for HumanVerificationDetails {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HumanVerificationDetails")
            .field("methods", &self.methods)
            .field("token", &Masked(&self.token))
            .finish()
    }
}

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct DeviceVerificationDetails {
    #[serde(rename = "ChallengeType")]
    challenge_type: i32,
    #[serde(rename = "ChallengePayload")]
    challenge_payload: String,
}

impl DeviceVerificationDetails {
    #[must_use]
    pub fn challenge_type(&self) -> i32 {
        self.challenge_type
    }

    #[must_use]
    pub fn challenge_payload(&self) -> &str {
        &self.challenge_payload
    }
}

impl fmt::Debug for DeviceVerificationDetails {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceVerificationDetails")
            .field("challenge_type", &self.challenge_type)
            .field("challenge_payload", &Masked(&self.challenge_payload))
            .finish()
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Details {
    #[serde(rename = "MissingScopes")]
    pub(crate) missing_scopes: Vec<String>,
}
