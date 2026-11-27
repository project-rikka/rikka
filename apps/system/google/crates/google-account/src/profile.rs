use rikka_net::Client;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::grants::Grants;
use crate::service::Service;

const USERINFO: &str = "https://openidconnect.googleapis.com/v1/userinfo";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    #[serde(rename = "sub")]
    pub id: String,
    pub email: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub picture: Option<String>,
}

impl Profile {
    pub async fn fetch(http: &Client, grants: &Grants) -> Result<Profile> {
        Profile::fetch_from(http, grants, USERINFO).await
    }

    pub async fn fetch_from(http: &Client, grants: &Grants, userinfo: &str) -> Result<Profile> {
        let response = grants
            .send(Service::Identity, |token| {
                http.get(userinfo).bearer_auth(token)
            })
            .await?
            .error_for_status()
            .await?;
        let profile: Profile = response.json().await?;
        if profile.id.is_empty() || profile.email.is_empty() {
            return Err(Error::Malformed("profile"));
        }
        Ok(profile)
    }
}
