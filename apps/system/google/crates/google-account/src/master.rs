use std::time::{SystemTime, UNIX_EPOCH};

use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::endpoint::{Endpoint, PLAY_SERVICES, SDK, SIGNATURE};
use crate::error::Result;
use crate::grants::AccessToken;
use crate::secret::Secret;
use crate::service::Service;

pub const SIGN_IN_PAGE: &str = "https://accounts.google.com/EmbeddedSetup";
pub const SIGN_IN_COOKIE_SITE: &str = "https://accounts.google.com/";
pub const SIGN_IN_COOKIE: &str = "oauth_token";

const ACCOUNT_APP: &str = "com.google.android.gms";
const LIFETIME: u64 = 3600;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Master {
    email: String,
    token: Secret,
    device: String,
}

impl Master {
    pub async fn sign_in(endpoint: &Endpoint, oauth_token: &str) -> Result<Master> {
        let device = rikka_types::hex::encode(device_id());
        let fields = endpoint
            .ask(&[
                ("lang", "en-US"),
                ("google_play_services_version", PLAY_SERVICES),
                ("sdk_version", SDK),
                ("device_country", "us"),
                ("operatorCountry", "us"),
                ("service", "ac2dm"),
                ("get_accountid", "1"),
                ("ACCESS_TOKEN", "1"),
                ("has_permission", "1"),
                ("add_account", "1"),
                ("source", "android"),
                ("androidId", &device),
                ("callerPkg", ACCOUNT_APP),
                ("Token", oauth_token.trim()),
                ("client_sig", SIGNATURE),
                ("callerSig", SIGNATURE),
                ("droidguard_results", "null"),
            ])
            .await?;
        Ok(Master {
            email: fields.need("Email")?.to_owned(),
            token: Secret::new(fields.need("Token")?),
            device,
        })
    }

    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    pub(crate) async fn mint(&self, endpoint: &Endpoint, service: Service) -> Result<AccessToken> {
        let fields = endpoint
            .ask(&[
                ("accountType", "HOSTED_OR_GOOGLE"),
                ("Email", &self.email),
                ("has_permission", "1"),
                ("EncryptedPasswd", self.token.expose()),
                ("service", service.scope()),
                ("source", "android"),
                ("androidId", &self.device),
                ("app", service.app()),
                ("client_sig", SIGNATURE),
                ("device_country", "us"),
                ("operatorCountry", "us"),
                ("lang", "en"),
                ("sdk_version", SDK),
                ("google_play_services_version", PLAY_SERVICES),
            ])
            .await?;
        let expires_at = fields
            .get("Expiry")
            .and_then(|expiry| expiry.parse().ok())
            .unwrap_or_else(|| now() + LIFETIME);
        Ok(AccessToken::new(fields.need("Auth")?, expires_at))
    }
}

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn device_id() -> [u8; 8] {
    let mut id = [0; 8];
    rand::rng().fill_bytes(&mut id);
    id
}
