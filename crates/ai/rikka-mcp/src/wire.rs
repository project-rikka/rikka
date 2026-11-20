use std::collections::BTreeMap;

use crate::http::Http;
use crate::oauth::Oauth;
use crate::server::Server;
use crate::stdio::Stdio;
use crate::transport::Transport;

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct Wire {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,

    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub(crate) kind: Option<String>,

    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub(crate) command: String,
    #[serde(default, skip_serializing_if = "<[String]>::is_empty")]
    pub(crate) args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) env: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) cwd: Option<String>,

    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub(crate) url: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) headers: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) oauth: Option<WireOauth>,

    #[serde(default, skip_serializing_if = "is_used")]
    pub(crate) disabled: bool,

    #[serde(flatten)]
    pub(crate) rest: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WireOauth {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    client_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    client_secret: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    callback_port: Option<u16>,
}

impl WireOauth {
    fn settings(&self) -> Oauth {
        let mut oauth = Oauth::new();
        if let Some(client) = &self.client_id {
            oauth = oauth.client(client.trim());
        }
        if let Some(secret) = &self.client_secret {
            oauth = oauth.secret(secret.as_str());
        }
        if let Some(port) = self.callback_port {
            oauth = oauth.port(port);
        }
        oauth
    }

    fn from_oauth(oauth: &Oauth) -> WireOauth {
        WireOauth {
            client_id: oauth.client_id().map(str::to_owned),
            client_secret: oauth.client_secret().map(str::to_owned),
            callback_port: oauth.callback_port(),
        }
    }
}

impl Wire {
    pub(crate) fn transport(&self) -> Transport {
        let named = self.kind.as_deref().map(str::trim);
        let remote = match named {
            Some(kind) => matches!(
                kind.to_ascii_lowercase().as_str(),
                "http" | "https" | "sse" | "streamable-http" | "streamable_http"
            ),
            None => !self.url.trim().is_empty(),
        };

        if remote {
            let mut http = Http::new(self.url.trim()).with_headers(self.headers.clone());
            if let Some(oauth) = &self.oauth {
                http = http.with_oauth(oauth.settings());
            }
            return http.into();
        }

        let mut stdio = Stdio::new(self.command.trim())
            .args(self.args.clone())
            .env(self.env.clone());
        if let Some(cwd) = self
            .cwd
            .as_deref()
            .map(str::trim)
            .filter(|it| !it.is_empty())
        {
            stdio = stdio.cwd(cwd);
        }
        stdio.into()
    }
}

impl From<Wire> for Server {
    fn from(wire: Wire) -> Server {
        Server::new(wire.name.clone().unwrap_or_default(), wire.transport())
            .with_disabled(wire.disabled)
            .with_extra(wire.rest)
    }
}

impl From<Server> for Wire {
    fn from(server: Server) -> Wire {
        let mut wire = Wire {
            name: Some(server.name().to_owned()),
            disabled: server.is_disabled(),
            rest: server.extra().clone(),
            ..Wire::default()
        };

        match server.transport() {
            Transport::Stdio(stdio) => {
                wire.command = stdio.command().to_owned();
                wire.args = stdio.arguments().to_vec();
                wire.env = stdio.variables().clone();
                wire.cwd = stdio.folder().map(|folder| folder.display().to_string());
            }
            Transport::Http(http) => {
                wire.kind = Some("http".to_owned());
                wire.url = http.url().to_owned();
                wire.headers = http.headers().clone();
                wire.oauth = http.oauth().map(WireOauth::from_oauth);
            }
        }

        wire
    }
}

fn is_used(disabled: &bool) -> bool {
    !*disabled
}
