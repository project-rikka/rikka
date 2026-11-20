use rikka_net::{HeaderMap, RequestBuilder, header};
use rikka_oauth::{Client, Loopback, Refresh, Token};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use rikka_llm::wire::text_of;
use rikka_llm::{Error, Result};
use rikka_llm_adapter::{BrowserLogin, Credentials};

const AUTHORIZE: &str = "https://claude.ai/oauth/authorize";
const TOKEN: &str = "https://platform.claude.com/v1/oauth/token";
const PROFILE: &str = "https://api.anthropic.com/api/oauth/profile";
const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const PORT: u16 = 54545;
const PATH: &str = "/callback";
const SCOPE: &str =
    "user:profile user:inference user:sessions:claude_code user:mcp_servers user:file_upload";
const ACCEPT: &str = "application/json, text/plain, */*";
const AGENT: &str = "axios/1.15.2";

#[derive(Clone, Serialize, Deserialize)]
pub struct ClaudeCodeCredentials {
    #[serde(flatten)]
    token: Token,
    #[serde(default)]
    email: String,
    #[serde(default)]
    account_uuid: String,
    #[serde(default)]
    organization_uuid: String,
    #[serde(default)]
    organization_name: String,
}

impl ClaudeCodeCredentials {
    #[must_use]
    pub(crate) fn email(&self) -> &str {
        &self.email
    }

    #[must_use]
    pub(crate) fn organization_name(&self) -> &str {
        &self.organization_name
    }

    fn from_token(token: Token) -> ClaudeCodeCredentials {
        let body = token.body();
        let account = body.get("account");
        let organization = body.get("organization");

        ClaudeCodeCredentials {
            email: nested(account, "email_address"),
            account_uuid: nested(account, "uuid"),
            organization_uuid: nested(organization, "uuid"),
            organization_name: nested(organization, "name"),
            token,
        }
    }

    async fn named(mut self, client: &Client) -> ClaudeCodeCredentials {
        let Ok(profile) = profile(client, self.token.access_token()).await else {
            return self;
        };

        let account = profile.get("account");
        let organization = profile.get("organization");

        fill(&mut self.email, nested(account, "email"));
        fill(&mut self.account_uuid, nested(account, "uuid"));
        fill(&mut self.organization_uuid, nested(organization, "uuid"));
        fill(&mut self.organization_name, nested(organization, "name"));
        self
    }
}

impl Refresh for ClaudeCodeCredentials {
    type Error = rikka_llm::Error;

    fn expires_at(&self) -> Option<u64> {
        self.token.expires_at()
    }

    async fn refresh(&self) -> Result<ClaudeCodeCredentials> {
        let refresh_token = self.token.refresh_token().ok_or(Error::Expired)?;
        let client = client()?;
        let token = client
            .refresh(refresh_token)
            .field("scope", SCOPE)
            .send()
            .await?
            .fill_missing_from(&self.token);

        let mut renewed = ClaudeCodeCredentials::from_token(token)
            .named(&client)
            .await;
        fill(&mut renewed.email, self.email.clone());
        fill(&mut renewed.account_uuid, self.account_uuid.clone());
        fill(
            &mut renewed.organization_uuid,
            self.organization_uuid.clone(),
        );
        fill(
            &mut renewed.organization_name,
            self.organization_name.clone(),
        );
        Ok(renewed)
    }
}

impl Credentials for ClaudeCodeCredentials {
    fn headers(&self) -> Result<HeaderMap> {
        Ok(rikka_oauth::bearer(self.token.access_token())?)
    }
}

pub(crate) fn login() -> BrowserLogin<ClaudeCodeCredentials> {
    BrowserLogin::loopback(
        client,
        Loopback::localhost(PORT).path(PATH),
        AUTHORIZE,
        |request| request.scope(SCOPE).pkce().param("code", "true"),
        |authorized| async move {
            let sent = authorized.authorization.state();
            let (code, state) = match authorized.callback.split_once('#') {
                Some((code, state)) if !state.is_empty() => (code, state),
                Some((code, _)) => (code, sent),
                None => (authorized.callback.as_str(), sent),
            };

            let token = authorized
                .client
                .exchange(code, &authorized.authorization)
                .field("state", state)
                .send()
                .await?;
            Ok(ClaudeCodeCredentials::from_token(token)
                .named(&authorized.client)
                .await)
        },
    )
}

fn client() -> Result<Client> {
    Ok(Client::builder(CLIENT_ID, TOKEN)
        .json()
        .header(header::ACCEPT, ACCEPT)
        .header(header::USER_AGENT, AGENT)
        .build()?)
}

async fn profile(client: &Client, access_token: &str) -> Result<Value> {
    Ok(rikka_oauth::send(
        native(client.http().get(PROFILE))
            .bearer_auth(access_token)
            .header(header::CACHE_CONTROL, "no-cache"),
    )
    .await?)
}

fn native(request: RequestBuilder) -> RequestBuilder {
    request
        .header(header::ACCEPT, ACCEPT)
        .header(header::USER_AGENT, AGENT)
}

fn nested(value: Option<&Value>, field: &str) -> String {
    value.map(|value| text_of(value, field)).unwrap_or_default()
}

fn fill(current: &mut String, fresh: String) {
    if current.is_empty() {
        *current = fresh;
    }
}

impl std::fmt::Debug for ClaudeCodeCredentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ClaudeCodeCredentials")
            .field("email", &self.email)
            .field("organization_name", &self.organization_name)
            .field("expires_at", &self.token.expires_at())
            .finish_non_exhaustive()
    }
}
