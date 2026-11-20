use crate::config_error::ServerConfigError;
use crate::error::{Error, Result};
use crate::expand::Expander;
use crate::http::Http;
use crate::oauth::Oauth;
use crate::stdio::Stdio;
use crate::transport::Transport;
use crate::wire::Wire;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(from = "Wire", into = "Wire")]
pub struct Server {
    name: String,
    transport: Transport,
    disabled: bool,
    rest: serde_json::Map<String, serde_json::Value>,
}

impl Server {
    #[must_use]
    pub fn new(name: impl Into<String>, transport: impl Into<Transport>) -> Server {
        Server {
            name: name.into(),
            transport: transport.into(),
            disabled: false,
            rest: serde_json::Map::new(),
        }
    }

    #[must_use]
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn transport(&self) -> &Transport {
        &self.transport
    }

    #[must_use]
    pub fn with_extra(mut self, rest: serde_json::Map<String, serde_json::Value>) -> Self {
        self.rest = rest;
        self
    }

    #[must_use]
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    #[must_use]
    pub fn extra(&self) -> &serde_json::Map<String, serde_json::Value> {
        &self.rest
    }

    pub fn expanded(&self, lookup: impl Fn(&str) -> Option<String>) -> Result<Server> {
        let mut expander = Expander::new(&lookup);
        let transport: Transport = match &self.transport {
            Transport::Stdio(stdio) => {
                let mut expanded = Stdio::new(expander.expand(stdio.command()))
                    .args(stdio.arguments().iter().map(|arg| expander.expand(arg)))
                    .env(
                        stdio
                            .variables()
                            .iter()
                            .map(|(name, value)| (name.clone(), expander.expand(value)))
                            .collect::<Vec<_>>(),
                    );
                if let Some(folder) = stdio.folder() {
                    expanded = expanded.cwd(expander.expand(&folder.to_string_lossy()));
                }
                expanded.into()
            }
            Transport::Http(http) => {
                let mut expanded = Http::new(expander.expand(http.url())).with_headers(
                    http.headers()
                        .iter()
                        .map(|(name, value)| (name.clone(), expander.expand(value)))
                        .collect::<Vec<_>>(),
                );
                if let Some(oauth) = http.oauth() {
                    let mut settings = Oauth::new();
                    if let Some(client) = oauth.client_id() {
                        settings = settings.client(expander.expand(client));
                    }
                    if let Some(secret) = oauth.client_secret() {
                        settings = settings.secret(expander.expand(secret));
                    }
                    if let Some(port) = oauth.callback_port() {
                        settings = settings.port(port);
                    }
                    expanded = expanded.with_oauth(settings);
                }
                expanded.into()
            }
        };

        let unset = expander.unset();
        if !unset.is_empty() {
            return Err(Error::Unset { names: unset });
        }
        Ok(Server {
            transport,
            ..self.clone()
        })
    }

    #[must_use]
    pub fn config_error(&self) -> Option<ServerConfigError> {
        if self.name.trim().is_empty() {
            return Some(ServerConfigError::MissingName);
        }
        match &self.transport {
            Transport::Stdio(stdio) if stdio.command().trim().is_empty() => {
                Some(ServerConfigError::MissingCommand)
            }
            Transport::Http(http) if http.url().trim().is_empty() => {
                Some(ServerConfigError::MissingUrl)
            }
            Transport::Http(http) if !is_web(http.url()) => Some(ServerConfigError::NonHttpUrl),
            _ => None,
        }
    }
}

fn is_web(url: &str) -> bool {
    let scheme = url.split_once("://").map(|(scheme, _)| scheme);
    matches!(scheme, Some(scheme) if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"))
}
