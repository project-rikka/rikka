use std::time::{SystemTime, UNIX_EPOCH};

use md5::{Digest, Md5};
use music_core::{Error, Id, Result};
use music_stream::{answer, api_client};
use rikka_net::{Client, RequestBuilder, Response};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::bundle::App;

const API: &str = "https://www.qobuz.com/api.json/0.2/";
const AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:120.0) Gecko/20100101 Firefox/120.0";
const PROFILE: &str = "qbz-1";
const RENEWAL: u64 = 60;
const BAD_REQUEST: u16 = 400;

pub(crate) fn http() -> Client {
    api_client(AGENT)
}

pub(crate) struct Api {
    http: Client,
    app: App,
    secret: String,
    token: String,
    session: Mutex<Option<Session>>,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Session {
    id: String,
    pub(crate) infos: String,
    expires: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Signed {
    WithArguments,
    Bare,
}

impl Api {
    pub(crate) async fn open(http: Client, app: App, token: String) -> Result<(Api, Value)> {
        let user = login(&http, &app, &token).await?;
        let mut api = Api {
            http,
            app,
            secret: String::new(),
            token,
            session: Mutex::new(None),
        };
        let secrets = api.app.secrets.clone();
        for secret in secrets {
            api.secret = secret;
            if let Some(session) = api.start_session().await? {
                *api.session.get_mut() = Some(session);
                return Ok((api, user));
            }
        }
        Err(Error::Unreadable(
            "no secret of the Qobuz player is accepted".into(),
        ))
    }

    pub(crate) fn secret(&self) -> &str {
        &self.secret
    }

    pub(crate) async fn get(&self, method: &str, arguments: &[(&str, String)]) -> Result<Value> {
        self.call(method, arguments, Signed::WithArguments).await
    }

    pub(crate) async fn call(
        &self,
        method: &str,
        arguments: &[(&str, String)],
        signed: Signed,
    ) -> Result<Value> {
        let request = self.http.get(format!("{API}{method}"));
        self.queried(request, method, arguments, signed).await
    }

    pub(crate) async fn session(&self) -> Result<Session> {
        let mut cached = self.session.lock().await;
        if let Some(session) = cached
            .as_ref()
            .filter(|session| session.expires > now() + RENEWAL)
        {
            return Ok(session.clone());
        }
        let session = self
            .start_session()
            .await?
            .ok_or_else(|| Error::Refused("Qobuz refused to start a stream session".into()))?;
        *cached = Some(session.clone());
        Ok(session)
    }

    pub(crate) async fn file(&self, track: &Id, format: u32, session: &Session) -> Result<Value> {
        let method = "file/url";
        let arguments = [
            ("format_id", format.to_string()),
            ("intent", "stream".to_owned()),
            ("track_id", track.to_string()),
        ];
        let request = self
            .http
            .get(format!("{API}{method}"))
            .header("X-Session-Id", session.id.as_str());
        self.queried(request, method, &arguments, Signed::WithArguments)
            .await
    }

    async fn queried(
        &self,
        request: RequestBuilder,
        method: &str,
        arguments: &[(&str, String)],
        signed: Signed,
    ) -> Result<Value> {
        let response = self
            .authorized(request)
            .query(&self.signed(method, arguments, signed))
            .send()
            .await
            .map_err(Error::unreachable)?;
        answered(method, response).await
    }

    async fn start_session(&self) -> Result<Option<Session>> {
        let method = "session/start";
        let arguments = [("profile", PROFILE.to_owned())];
        let response = self
            .authorized(self.http.post(format!("{API}{method}")))
            .form(&self.signed(method, &arguments, Signed::WithArguments))
            .send()
            .await
            .map_err(Error::unreachable)?;
        if response.status().as_u16() == BAD_REQUEST {
            return Ok(None);
        }
        let answer = answered(method, response).await?;
        let field = |key: &str| {
            answer
                .get(key)
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        };
        Ok(Some(Session {
            id: field("session_id")
                .ok_or_else(|| Error::Unreadable("the stream session has no id".into()))?,
            infos: field("infos")
                .ok_or_else(|| Error::Unreadable("the stream session has no key salt".into()))?,
            expires: answer
                .get("expires_at")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
        }))
    }

    fn authorized(&self, request: RequestBuilder) -> RequestBuilder {
        request
            .header("X-App-Id", self.app.id.as_str())
            .header("X-User-Auth-Token", self.token.as_str())
    }

    fn signed(
        &self,
        method: &str,
        arguments: &[(&str, String)],
        signed: Signed,
    ) -> Vec<(String, String)> {
        let stamp = now().to_string();
        let signing = match signed {
            Signed::WithArguments => arguments,
            Signed::Bare => &[],
        };
        let signature = signature(method, signing, &stamp, &self.secret);
        arguments
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .chain([
                ("request_ts".to_owned(), stamp),
                ("request_sig".to_owned(), signature),
            ])
            .collect()
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Session")
            .field("expires", &self.expires)
            .finish_non_exhaustive()
    }
}

async fn answered(method: &str, response: Response) -> Result<Value> {
    let status = response.status().as_u16();
    let body = response.bytes().await.map_err(Error::unreachable)?;
    let answer = serde_json::from_slice::<Value>(&body);
    if (200..300).contains(&status) {
        return answer.map_err(|_| Error::Unreadable(format!("{method} answered with no JSON")));
    }
    let message = answer
        .ok()
        .and_then(|answer| answer.get("message")?.as_str().map(str::to_owned))
        .unwrap_or_else(|| "no reason given".to_owned());
    Err(Error::from_status(
        status,
        format!("{method} answered {status}: {message}"),
    ))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

pub(crate) async fn exchange(http: &Client, app: &App, code: &str) -> Result<String> {
    let key = app
        .private_key
        .as_deref()
        .ok_or_else(|| Error::Unreadable("the Qobuz player has no sign-in key".into()))?;
    let request = http
        .get(format!("{API}oauth/callback"))
        .header("X-App-Id", app.id.as_str())
        .query(&[("code", code), ("private_key", key), ("app_id", &app.id)]);
    let session: Value = answer(request)
        .await?
        .json()
        .await
        .map_err(|error| Error::Unreadable(error.to_string()))?;
    session
        .get("token")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| Error::Refused("Qobuz returned no sign-in token".into()))
}

async fn login(http: &Client, app: &App, token: &str) -> Result<Value> {
    let request = http
        .post(format!("{API}user/login"))
        .header("X-App-Id", app.id.as_str())
        .header("X-User-Auth-Token", token)
        .text("extra=partner");
    match answer(request).await {
        Ok(response) => response
            .json()
            .await
            .map_err(|error| Error::Unreadable(error.to_string())),
        Err(Error::Refused(_)) => Err(Error::Refused("the Qobuz sign-in has expired".into())),
        Err(error) => Err(error),
    }
}

fn signature(method: &str, arguments: &[(&str, String)], stamp: &str, secret: &str) -> String {
    let mut sorted: Vec<&(&str, String)> = arguments.iter().collect();
    sorted.sort_by_key(|(key, _)| *key);
    let mut hasher = Md5::new();
    hasher.update(method.replace('/', ""));
    for (key, value) in sorted {
        hasher.update(key);
        hasher.update(value);
    }
    hasher.update(stamp);
    hasher.update(secret);
    rikka_types::hex::encode(hasher.finalize())
}
