use std::time::Duration;

use rikka_net::{HeaderMap, HeaderName, RequestBuilder, header};
use rikka_oauth::{Client, Loopback, Refresh, Token};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use rikka_llm::wire::{self, text_of};
use rikka_llm::{Error, Result};
use rikka_llm_adapter::{BrowserLogin, Credentials};

use crate::hub;

const AUTHORIZE: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN: &str = "https://oauth2.googleapis.com/token";
const CLIENT_ID: &str = "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com";
const CLIENT_SECRET: &str = "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";
const PORT: u16 = 51121;
const PATH: &str = "/oauth-callback";
const SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform \
https://www.googleapis.com/auth/userinfo.email \
https://www.googleapis.com/auth/userinfo.profile \
https://www.googleapis.com/auth/aicode \
https://www.googleapis.com/auth/cclog \
https://www.googleapis.com/auth/experimentsandconfigs";

const CODE_ASSIST: &str = "https://daily-cloudcode-pa.googleapis.com/v1internal:loadCodeAssist";
const ONBOARD: &str = "https://daily-cloudcode-pa.googleapis.com/v1internal:onboardUser";
const ONBOARD_ATTEMPTS: usize = 5;
const ONBOARD_INTERVAL: Duration = Duration::from_secs(2);

const GOOG_AGENT: &str = "gl-node/22.21.1";
const RENEWAL_AGENT: &str = "Go-http-client/2.0";

#[derive(Clone, Serialize, Deserialize)]
pub struct AntigravityCredentials {
    #[serde(flatten)]
    token: Token,
    #[serde(default)]
    project_id: String,
}

impl AntigravityCredentials {
    #[must_use]
    pub(crate) fn email(&self) -> &str {
        self.token.identity().text(&["email"])
    }

    #[must_use]
    pub(crate) fn project_id(&self) -> &str {
        &self.project_id
    }
}

impl Refresh for AntigravityCredentials {
    type Error = rikka_llm::Error;

    fn expires_at(&self) -> Option<u64> {
        self.token.expires_at()
    }

    async fn refresh(&self) -> Result<AntigravityCredentials> {
        let refresh_token = self.token.refresh_token().ok_or(Error::Expired)?;
        let token = client()?
            .refresh(refresh_token)
            .header(header::USER_AGENT, RENEWAL_AGENT)
            .send()
            .await?
            .fill_missing_from(&self.token);

        Ok(AntigravityCredentials {
            token,
            project_id: self.project_id.clone(),
        })
    }
}

impl Credentials for AntigravityCredentials {
    fn headers(&self) -> Result<HeaderMap> {
        wire::headers()
            .user_agent(&hub::agent())
            .extend(rikka_oauth::bearer(self.token.access_token())?)
            .build()
    }
}

pub(crate) fn login() -> BrowserLogin<AntigravityCredentials> {
    BrowserLogin::loopback(
        client,
        Loopback::localhost(PORT).path(PATH),
        AUTHORIZE,
        |request| {
            request
                .scope(SCOPE)
                .param("access_type", "offline")
                .param("prompt", "consent")
        },
        |authorized| async move {
            hub::refresh().await;
            let token = authorized
                .client
                .exchange(&authorized.callback, &authorized.authorization)
                .send()
                .await?;
            Ok(AntigravityCredentials {
                project_id: project(&authorized.client, token.access_token()).await?,
                token,
            })
        },
    )
}

fn client() -> Result<Client> {
    Ok(Client::builder(CLIENT_ID, TOKEN)
        .secret(CLIENT_SECRET)
        .build()?)
}

async fn project(client: &Client, access_token: &str) -> Result<String> {
    let body = rikka_oauth::send(
        control(client.http().post(CODE_ASSIST), access_token, &hub::agent())
            .json(&json!({ "metadata": { "ideType": "ANTIGRAVITY" } })),
    )
    .await?;

    if let Some(project) = named(&body) {
        return Ok(project);
    }

    onboard(client, access_token, &tier(&body)).await
}

async fn onboard(client: &Client, access_token: &str, tier: &str) -> Result<String> {
    let body = json!({
        "tier_id": tier,
        "metadata": {
            "ide_type": "ANTIGRAVITY",
            "ide_version": hub::version(),
            "ide_name": "antigravity",
        },
    });

    for _ in 0..ONBOARD_ATTEMPTS {
        let answer = rikka_oauth::send(
            control(
                client.http().post(ONBOARD),
                access_token,
                &hub::onboarding_agent(),
            )
            .header(header::ACCEPT, "*/*")
            .header(HeaderName::from_static("x-goog-api-client"), GOOG_AGENT)
            .json(&body),
        )
        .await?;

        if answer.get("done").and_then(Value::as_bool) == Some(true) {
            return answer
                .get("response")
                .and_then(named)
                .ok_or_else(|| Error::Login("the account was onboarded to no project".to_owned()));
        }

        tokio::time::sleep(ONBOARD_INTERVAL).await;
    }

    Err(Error::Login(
        "the account was still being onboarded after five attempts".to_owned(),
    ))
}

fn control(request: RequestBuilder, access_token: &str, agent: &str) -> RequestBuilder {
    request
        .bearer_auth(access_token)
        .header(header::USER_AGENT, agent)
}

fn named(body: &Value) -> Option<String> {
    ["cloudaicompanionProject", "projectId", "project"]
        .into_iter()
        .find_map(|key| match body.get(key) {
            Some(Value::String(id)) if !id.trim().is_empty() => Some(id.trim().to_owned()),
            Some(Value::Object(project)) => project
                .get("id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(str::to_owned),
            _ => None,
        })
}

fn tier(body: &Value) -> String {
    let default = body
        .get("allowedTiers")
        .and_then(Value::as_array)
        .and_then(|tiers| {
            tiers
                .iter()
                .find(|tier| tier.get("isDefault").and_then(Value::as_bool) == Some(true))
        })
        .map(|tier| text_of(tier, "id"))
        .filter(|id| !id.is_empty());

    let current = body
        .get("currentTier")
        .map(|tier| text_of(tier, "id"))
        .filter(|id| !id.is_empty());

    default
        .or(current)
        .unwrap_or_else(|| "free-tier".to_owned())
}

impl std::fmt::Debug for AntigravityCredentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AntigravityCredentials")
            .field("email", &self.email())
            .field("project_id", &self.project_id)
            .field("expires_at", &self.token.expires_at())
            .finish_non_exhaustive()
    }
}
