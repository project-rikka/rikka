use rikka_net::{Client, Response, header};
use rikka_sync::Mutex;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::device_key::DeviceKey;
use crate::error::{Error, Result};
use crate::microsoft::CLIENT_ID;

const DEVICE_AUTHENTICATE: &str = "https://device.auth.xboxlive.com/device/authenticate";
const SISU_AUTHORIZE: &str = "https://sisu.xboxlive.com/authorize";
const XSTS_AUTHORIZE: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MINECRAFT_RELYING_PARTY: &str = "rp://api.minecraftservices.com/";
const RENEW_BEFORE: i64 = 60 * 60;
const SKEW_TOLERANCE: i64 = 60;

#[derive(Clone, Debug, Default, Deserialize)]
struct UserClaim {
    #[serde(default)]
    uhs: String,
    #[serde(default)]
    xid: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct Claims {
    #[serde(default)]
    xui: Vec<UserClaim>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XboxToken {
    token: String,
    #[serde(default)]
    not_after: Option<String>,
    #[serde(default)]
    display_claims: Claims,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SisuAuthorization {
    title_token: XboxToken,
    user_token: XboxToken,
    authorization_token: XboxToken,
}

#[derive(Debug, Deserialize)]
struct XboxError {
    #[serde(rename = "XErr", default)]
    code: u64,
}

pub(crate) struct Identity {
    pub(crate) token: String,
    pub(crate) user_hash: String,
    pub(crate) xuid: String,
}

struct Cached {
    token: String,
    not_after: i64,
}

pub(crate) struct Xbox {
    http: Client,
    key: DeviceKey,
    device: Mutex<Option<Cached>>,
    skew: Mutex<i64>,
}

enum Step {
    Device,
    Sisu,
    Xsts,
}

impl Step {
    const fn name(&self) -> &'static str {
        match self {
            Step::Device => "Xbox device authentication",
            Step::Sisu => "Xbox user authorization",
            Step::Xsts => "Xbox security token",
        }
    }

    const fn path(&self) -> &'static str {
        match self {
            Step::Device => "/device/authenticate",
            Step::Sisu => "/authorize",
            Step::Xsts => "/xsts/authorize",
        }
    }

    const fn url(&self) -> &'static str {
        match self {
            Step::Device => DEVICE_AUTHENTICATE,
            Step::Sisu => SISU_AUTHORIZE,
            Step::Xsts => XSTS_AUTHORIZE,
        }
    }
}

fn now() -> i64 {
    jiff::Timestamp::now().as_second()
}

fn server_time(response: &Response) -> Option<i64> {
    let date = response.headers().get(header::DATE)?.to_str().ok()?;
    jiff::fmt::rfc2822::parse(date)
        .ok()
        .map(|zoned| zoned.timestamp().as_second())
}

impl Xbox {
    pub(crate) fn new(http: Client, key: DeviceKey) -> Xbox {
        Xbox {
            http,
            key,
            device: Mutex::new(None),
            skew: Mutex::new(0),
        }
    }

    async fn signed<T: DeserializeOwned>(&self, step: Step, body: &Value) -> Result<T> {
        let bytes = serde_json::to_vec(body).map_err(|source| Error::Response {
            step: step.name(),
            source,
        })?;
        let mut retried = false;
        loop {
            let skew = *self.skew.lock();
            let signature = self.key.sign(step.path(), "", &bytes, now() + skew);
            let mut request = self
                .http
                .post(step.url())
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .header(header::ACCEPT, "application/json")
                .header("Signature", signature)
                .body(bytes.clone());
            if !matches!(step, Step::Sisu) {
                request = request.header("x-xbl-contract-version", "1");
            }
            let response = request.send().await?;
            let status = response.status();
            if let Some(server) = server_time(&response) {
                let measured = server - now();
                let drifted = (measured - skew).abs() > SKEW_TOLERANCE;
                *self.skew.lock() = measured;
                if !status.is_success() && drifted && !retried {
                    retried = true;
                    continue;
                }
            }
            let text = response.text().await?;
            if !status.is_success() {
                return Err(rejection(step.name(), status.as_u16(), &text));
            }
            return serde_json::from_str(&text).map_err(|source| Error::Response {
                step: step.name(),
                source,
            });
        }
    }

    async fn device_token(&self) -> Result<String> {
        if let Some(cached) = self.device.lock().as_ref()
            && cached.not_after - RENEW_BEFORE > now()
        {
            return Ok(cached.token.clone());
        }
        let body = json!({
            "Properties": {
                "AuthMethod": "ProofOfPossession",
                "Id": self.key.id(),
                "DeviceType": "Win32",
                "Version": "10.16.0",
                "ProofKey": self.key.proof_key(),
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        });
        let token: XboxToken = self.signed(Step::Device, &body).await?;
        let not_after = token
            .not_after
            .as_deref()
            .and_then(|text| text.parse::<jiff::Timestamp>().ok())
            .map_or(now() + RENEW_BEFORE * 2, |instant| instant.as_second());
        *self.device.lock() = Some(Cached {
            token: token.token.clone(),
            not_after,
        });
        Ok(token.token)
    }

    pub(crate) async fn authorize(&self, microsoft_token: &str) -> Result<Identity> {
        let device = self.device_token().await?;
        let sisu: SisuAuthorization = self
            .signed(
                Step::Sisu,
                &json!({
                    "AccessToken": format!("t={microsoft_token}"),
                    "AppId": CLIENT_ID,
                    "DeviceToken": device,
                    "ProofKey": self.key.proof_key(),
                    "Sandbox": "RETAIL",
                    "SiteName": "user.auth.xboxlive.com",
                    "RelyingParty": "http://xboxlive.com",
                    "UseModernGamertag": true,
                }),
            )
            .await?;
        let player = sisu
            .authorization_token
            .display_claims
            .xui
            .first()
            .cloned()
            .unwrap_or_default();
        let xsts: XboxToken = self
            .signed(
                Step::Xsts,
                &json!({
                    "RelyingParty": MINECRAFT_RELYING_PARTY,
                    "TokenType": "JWT",
                    "Properties": {
                        "SandboxId": "RETAIL",
                        "UserTokens": [sisu.user_token.token],
                        "DeviceToken": device,
                        "TitleToken": sisu.title_token.token,
                    },
                }),
            )
            .await?;
        let user_hash = xsts
            .display_claims
            .xui
            .first()
            .map(|claim| claim.uhs.clone())
            .filter(|hash| !hash.is_empty())
            .ok_or(Error::NoXboxAccount)?;
        Ok(Identity {
            token: xsts.token,
            user_hash,
            xuid: player.xid.unwrap_or_default(),
        })
    }
}

fn rejection(step: &'static str, status: u16, body: &str) -> Error {
    let code = serde_json::from_str::<XboxError>(body).map_or(0, |error| error.code);
    match code {
        0 => Error::Rejected { step, status },
        2_148_916_233 => Error::NoXboxAccount,
        2_148_916_235 => Error::XboxUnavailable,
        2_148_916_238 => Error::ChildAccount,
        code => Error::Xbox(code),
    }
}
