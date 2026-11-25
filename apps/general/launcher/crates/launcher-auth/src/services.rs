use rikka_net::{Client, Form, Part, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{Error, Result};
use crate::random;
use crate::xbox::Identity;

const SERVICES: &str = "https://api.minecraftservices.com";
const PROFILE: &str = "https://api.minecraftservices.com/minecraft/profile";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SkinVariant {
    Classic,
    Slim,
}

impl SkinVariant {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            SkinVariant::Classic => "classic",
            SkinVariant::Slim => "slim",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum TextureState {
    Active,
    Inactive,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Skin {
    pub id: String,
    pub state: TextureState,
    pub url: String,
    pub variant: SkinVariant,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Cape {
    pub id: String,
    pub state: TextureState,
    pub url: String,
    #[serde(default)]
    pub alias: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub skins: Vec<Skin>,
    #[serde(default)]
    pub capes: Vec<Cape>,
}

impl Profile {
    #[must_use]
    pub fn active_skin(&self) -> Option<&Skin> {
        self.skins
            .iter()
            .find(|skin| skin.state == TextureState::Active)
    }

    #[must_use]
    pub fn active_cape(&self) -> Option<&Cape> {
        self.capes
            .iter()
            .find(|cape| cape.state == TextureState::Active)
    }
}

#[derive(Deserialize)]
struct Login {
    access_token: String,
    expires_in: i64,
}

#[derive(Deserialize)]
struct Entitlement {
    name: String,
}

#[derive(Deserialize)]
struct Entitlements {
    #[serde(default)]
    items: Vec<Entitlement>,
}

pub(crate) struct MinecraftToken {
    pub(crate) access_token: String,
    pub(crate) expires_in: i64,
}

#[derive(Clone, Debug)]
pub struct Services {
    http: Client,
}

async fn read<T: DeserializeOwned>(response: rikka_net::Response, step: &'static str) -> Result<T> {
    let status = response.status();
    if status == StatusCode::TOO_MANY_REQUESTS {
        return Err(Error::RateLimited);
    }
    let text = response.text().await?;
    if !status.is_success() {
        return Err(Error::Rejected {
            step,
            status: status.as_u16(),
        });
    }
    serde_json::from_str(&text).map_err(|source| Error::Response { step, source })
}

impl Services {
    pub(crate) fn new(http: Client) -> Services {
        Services { http }
    }

    pub(crate) async fn login(&self, identity: &Identity) -> Result<MinecraftToken> {
        let response = self
            .http
            .post(format!("{SERVICES}/launcher/login"))
            .json(&json!({
                "platform": "PC_LAUNCHER",
                "xtoken": format!("XBL3.0 x={};{}", identity.user_hash, identity.token),
            }))
            .send()
            .await?;
        let login: Login = read(response, "Minecraft sign-in").await?;
        Ok(MinecraftToken {
            access_token: login.access_token,
            expires_in: login.expires_in,
        })
    }

    pub(crate) async fn check_license(&self, token: &str) -> Result<()> {
        let request = random::uuid()?;
        let response = self
            .http
            .get(format!("{SERVICES}/entitlements/license"))
            .query(&[("requestId", request)])
            .bearer_auth(token)
            .send()
            .await?;
        let entitlements: Entitlements = read(response, "Minecraft license").await?;
        let owned = entitlements
            .items
            .iter()
            .any(|item| item.name == "game_minecraft" || item.name == "product_minecraft");
        match owned {
            true => Ok(()),
            false => Err(Error::NoLicense),
        }
    }

    pub async fn profile(&self, token: &str) -> Result<Profile> {
        let response = self.http.get(PROFILE).bearer_auth(token).send().await?;
        if response.status() == StatusCode::NOT_FOUND {
            return Err(Error::NoProfile);
        }
        read(response, "Minecraft profile").await
    }

    pub async fn upload_skin(
        &self,
        token: &str,
        variant: SkinVariant,
        png: Vec<u8>,
    ) -> Result<Profile> {
        let form = Form::new().text("variant", variant.name()).part(
            "file",
            Part::bytes(png)
                .file_name("skin.png")
                .content_type("image/png"),
        );
        let response = self
            .http
            .post(format!("{PROFILE}/skins"))
            .bearer_auth(token)
            .multipart(form)
            .send()
            .await?;
        read(response, "skin upload").await
    }

    pub async fn reset_skin(&self, token: &str) -> Result<Profile> {
        let response = self
            .http
            .delete(format!("{PROFILE}/skins/active"))
            .bearer_auth(token)
            .send()
            .await?;
        read(response, "skin reset").await
    }

    pub async fn show_cape(&self, token: &str, cape: &str) -> Result<Profile> {
        let response = self
            .http
            .put(format!("{PROFILE}/capes/active"))
            .bearer_auth(token)
            .json(&json!({ "capeId": cape }))
            .send()
            .await?;
        read(response, "cape change").await
    }

    pub async fn hide_cape(&self, token: &str) -> Result<Profile> {
        let response = self
            .http
            .delete(format!("{PROFILE}/capes/active"))
            .bearer_auth(token)
            .send()
            .await?;
        read(response, "cape change").await
    }
}
