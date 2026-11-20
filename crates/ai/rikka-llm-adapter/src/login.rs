use futures_util::FutureExt;
use futures_util::future::BoxFuture;
use rikka_oauth::{Authorization, AuthorizationBuilder, Client, Loopback};
use serde::Serialize;
use serde_json::Value;

use crate::sign_in::SignInMethod;

#[derive(Clone, Debug)]
pub enum Prompt {
    Browser { url: String },
    Code { url: String, code: String },
}

impl Prompt {
    #[must_use]
    pub fn url(&self) -> &str {
        match self {
            Prompt::Browser { url } | Prompt::Code { url, .. } => url,
        }
    }
}

pub enum Grant<T> {
    Key(String),
    Tokens(T),
}

impl<T> From<T> for Grant<T> {
    fn from(tokens: T) -> Grant<T> {
        Grant::Tokens(tokens)
    }
}

impl<T> std::fmt::Debug for Grant<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Grant::Key(_) => "Grant::Key",
            Grant::Tokens(_) => "Grant::Tokens",
        })
    }
}

type Redeeming<T> = BoxFuture<'static, rikka_llm::Result<Grant<T>>>;

type Starting<T> = BoxFuture<'static, rikka_llm::Result<Login<T>>>;

pub struct Login<T> {
    prompt: Prompt,
    redeeming: Redeeming<T>,
}

#[must_use]
pub struct BrowserLogin<T>(Starting<T>);

#[must_use]
pub struct DeviceCodeLogin<T>(Starting<T>);

impl<T: Send + 'static> BrowserLogin<T> {
    pub fn start<F, S>(
        started: impl Future<Output = rikka_llm::Result<(String, F)>> + Send + 'static,
    ) -> BrowserLogin<T>
    where
        F: Future<Output = rikka_llm::Result<S>> + Send + 'static,
        S: Into<Grant<T>> + 'static,
    {
        BrowserLogin(
            started
                .map(|started| {
                    started
                        .map(|(url, redeeming)| Login::prompted(Prompt::Browser { url }, redeeming))
                })
                .boxed(),
        )
    }
}

pub struct Authorized {
    pub client: Client,
    pub authorization: Authorization,
    pub callback: String,
}

impl<T: Send + 'static> BrowserLogin<T> {
    pub fn loopback<F, S>(
        client: fn() -> rikka_llm::Result<Client>,
        loopback: Loopback,
        authorize: &'static str,
        request: for<'a> fn(AuthorizationBuilder<'a>) -> AuthorizationBuilder<'a>,
        redeem: fn(Authorized) -> F,
    ) -> BrowserLogin<T>
    where
        F: Future<Output = rikka_llm::Result<S>> + Send + 'static,
        S: Into<Grant<T>> + 'static,
    {
        BrowserLogin::start(async move {
            let client = client()?;
            let redirect = loopback.bind().await?;
            let authorization = request(client.authorization(authorize, redirect.uri())).build()?;
            let url = authorization.url().to_owned();
            let redeeming = async move {
                let callback = redirect.code(authorization.state()).await?;
                redeem(Authorized {
                    client,
                    authorization,
                    callback,
                })
                .await
            };
            Ok((url, redeeming))
        })
    }
}

impl<T: Send + 'static> DeviceCodeLogin<T> {
    pub fn start<F, S>(
        started: impl Future<Output = rikka_llm::Result<(String, String, F)>> + Send + 'static,
    ) -> DeviceCodeLogin<T>
    where
        F: Future<Output = rikka_llm::Result<S>> + Send + 'static,
        S: Into<Grant<T>> + 'static,
    {
        DeviceCodeLogin(
            started
                .map(|started| {
                    started.map(|(url, code, redeeming)| {
                        Login::prompted(Prompt::Code { url, code }, redeeming)
                    })
                })
                .boxed(),
        )
    }
}

impl<T> Login<T> {
    #[must_use]
    pub fn prompt(&self) -> &Prompt {
        &self.prompt
    }

    pub fn into_parts(self) -> (Prompt, Redeeming<T>) {
        (self.prompt, self.redeeming)
    }
}

impl<T: Send + 'static> Login<T> {
    fn prompted<S: Into<Grant<T>> + 'static>(
        prompt: Prompt,
        redeeming: impl Future<Output = rikka_llm::Result<S>> + Send + 'static,
    ) -> Login<T> {
        Login {
            prompt,
            redeeming: redeeming.map(|redeemed| redeemed.map(Into::into)).boxed(),
        }
    }
}

impl<T: Serialize + Send + 'static> SignInMethod<T> {
    pub(crate) fn start(&self) -> Option<Starting<Value>> {
        let started = match self {
            SignInMethod::Key { .. } => return None,
            SignInMethod::Browser(browser) | SignInMethod::KeyOrBrowser { browser, .. } => {
                browser().0
            }
            SignInMethod::Code(coded) => coded().0,
        };
        Some(started.map(|started| started.map(Login::erased)).boxed())
    }
}

impl<T: Serialize + Send + 'static> Login<T> {
    fn erased(self) -> Login<Value> {
        Login {
            prompt: self.prompt,
            redeeming: self
                .redeeming
                .map(|redeemed| {
                    redeemed.map(|signed| match signed {
                        Grant::Key(key) => Grant::Key(key),
                        Grant::Tokens(tokens) => Grant::Tokens(
                            serde_json::to_value(tokens)
                                .expect("credentials should serialize to JSON"),
                        ),
                    })
                })
                .boxed(),
        }
    }
}

impl<T> std::fmt::Debug for Login<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Login")
            .field("prompt", &self.prompt)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Authorized {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Authorized").finish_non_exhaustive()
    }
}

impl<T> std::fmt::Debug for BrowserLogin<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BrowserLogin")
            .finish_non_exhaustive()
    }
}

impl<T> std::fmt::Debug for DeviceCodeLogin<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeviceCodeLogin")
            .finish_non_exhaustive()
    }
}
