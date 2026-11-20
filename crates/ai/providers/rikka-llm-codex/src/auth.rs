use rikka_net::HeaderMap;
use rikka_oauth::{Client, Loopback, Refresh, Token};
use serde::{Deserialize, Serialize};

use rikka_llm::wire;
use rikka_llm::{Error, Result};
use rikka_llm_adapter::{BrowserLogin, Credentials};

const AUTHORIZE: &str = "https://auth.openai.com/oauth/authorize";
const TOKEN: &str = "https://auth.openai.com/oauth/token";
const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const PORT: u16 = 1455;
const PATH: &str = "/auth/callback";
const SCOPE: &str = "openid email profile offline_access";
const RENEWAL_SCOPE: &str = "openid profile email";

const AUTH_CLAIM: &str = "https://api.openai.com/auth";

#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CodexCredentials {
    token: Token,
}

impl CodexCredentials {
    #[must_use]
    pub(crate) fn email(&self) -> &str {
        self.token.identity().text(&["email"])
    }

    #[must_use]
    pub(crate) fn plan(&self) -> &str {
        self.token
            .identity()
            .text(&[AUTH_CLAIM, "chatgpt_plan_type"])
    }

    fn account_id(&self) -> &str {
        self.token
            .identity()
            .text(&[AUTH_CLAIM, "chatgpt_account_id"])
    }
}

impl Refresh for CodexCredentials {
    type Error = rikka_llm::Error;

    fn expires_at(&self) -> Option<u64> {
        self.token.expires_at()
    }

    async fn refresh(&self) -> Result<CodexCredentials> {
        let refresh_token = self.token.refresh_token().ok_or(Error::Expired)?;
        let token = client()?
            .refresh(refresh_token)
            .field("scope", RENEWAL_SCOPE)
            .send()
            .await?
            .fill_missing_from(&self.token);
        Ok(CodexCredentials { token })
    }
}

impl Credentials for CodexCredentials {
    fn headers(&self) -> Result<HeaderMap> {
        let mut headers = wire::headers().extend(rikka_oauth::bearer(self.token.access_token())?);
        let account_id = self.account_id();
        if !account_id.is_empty() {
            headers = headers.text("chatgpt-account-id", account_id, "the account id");
        }
        headers.build()
    }
}

pub(crate) fn login() -> BrowserLogin<CodexCredentials> {
    BrowserLogin::loopback(
        client,
        Loopback::localhost(PORT).path(PATH),
        AUTHORIZE,
        |request| {
            request
                .scope(SCOPE)
                .pkce()
                .param("prompt", "login")
                .param("id_token_add_organizations", "true")
                .param("codex_cli_simplified_flow", "true")
        },
        |authorized| async move {
            let token = authorized
                .client
                .exchange(&authorized.callback, &authorized.authorization)
                .send()
                .await?;
            Ok(CodexCredentials { token })
        },
    )
}

fn client() -> Result<Client> {
    Ok(Client::new(CLIENT_ID, TOKEN)?)
}

impl std::fmt::Debug for CodexCredentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CodexCredentials")
            .field("email", &self.email())
            .field("plan", &self.plan())
            .field("expires_at", &self.token.expires_at())
            .finish_non_exhaustive()
    }
}
