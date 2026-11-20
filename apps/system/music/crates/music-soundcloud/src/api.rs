use std::sync::Arc;

use music_core::{Error, Result, Shared};
use music_stream::api_client;
use rikka_net::Url;
use rikka_net::{Client, Method, header};
use serde_json::Value;

use crate::client_id::ClientIds;
use crate::links::Links;
use crate::session::Session;

pub(crate) const AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0";
const API: &str = "https://api-v2.soundcloud.com/";
const CLIENT_ID: &str = "client_id";
const UNAUTHORIZED: u16 = 401;

pub(crate) struct Api {
    http: Client,
    ids: Arc<ClientIds>,
    session: Session,
    links: Links,
}

enum Failure {
    Unauthorized(String),
    Other(Error),
}

#[derive(Clone, Copy, Debug)]
enum Renewal {
    Token,
    Client,
}

impl From<Error> for Failure {
    fn from(error: Error) -> Failure {
        Failure::Other(error)
    }
}

impl Api {
    pub(crate) fn new(ids: Arc<ClientIds>, session: Session) -> Api {
        Api {
            http: api_client(AGENT),
            ids,
            session,
            links: Links::default(),
        }
    }

    pub(crate) async fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value> {
        self.call(Method::GET, &address(path)?, query, None).await
    }

    pub(crate) async fn get_at(&self, url: &str, query: &[(&str, String)]) -> Result<Value> {
        let url = Url::parse(url).map_err(|error| Error::Unreadable(error.to_string()))?;
        self.call(Method::GET, &url, query, None).await
    }

    pub(crate) async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value> {
        self.call(method, &address(path)?, &[], body).await
    }

    pub(crate) async fn paginate_all(
        &self,
        path: &str,
        query: &[(&str, String)],
        most: usize,
    ) -> Result<Vec<Value>> {
        let mut page = self.get(path, query).await?;
        let mut items = Vec::new();
        loop {
            let arrived = match page.get_mut("collection").map(Value::take) {
                Some(Value::Array(arrived)) => arrived,
                _ => Vec::new(),
            };
            let exhausted = arrived.is_empty();
            items.extend(arrived);
            match page.get("next_href").and_then(Value::as_str) {
                Some(next) if items.len() < most && !exhausted => {
                    page = self.get_at(next, &[]).await?;
                }
                _ => break,
            }
        }
        items.truncate(most);
        Ok(items)
    }

    pub(crate) fn link(&self, shared: Shared<'_>) -> String {
        self.links.url_for(shared)
    }

    async fn call(
        &self,
        method: Method,
        url: &Url,
        query: &[(&str, String)],
        body: Option<&Value>,
    ) -> Result<Value> {
        let mut client = self.ids.current(&self.http).await?;
        let mut token = self.session.token().await;
        let mut refused = match self
            .attempt(&method, url, query, body, &client, &token)
            .await
        {
            Ok(answer) => return Ok(answer),
            Err(Failure::Other(error)) => return Err(error),
            Err(Failure::Unauthorized(message)) => message,
        };
        for renewal in [Renewal::Token, Renewal::Client] {
            tracing::debug!("SoundCloud request refused, retrying with {renewal:?}: {refused}");
            match renewal {
                Renewal::Token => {
                    if !self.session.refresh(&client, &token).await? {
                        continue;
                    }
                    token = self.session.token().await;
                }
                Renewal::Client => client = self.ids.renew(&self.http, &client).await?,
            }
            refused = match self
                .attempt(&method, url, query, body, &client, &token)
                .await
            {
                Ok(answer) => return Ok(answer),
                Err(Failure::Other(error)) => return Err(error),
                Err(Failure::Unauthorized(message)) => message,
            };
        }
        Err(Error::Refused(refused))
    }

    async fn attempt(
        &self,
        method: &Method,
        url: &Url,
        query: &[(&str, String)],
        body: Option<&Value>,
        client: &str,
        token: &str,
    ) -> Result<Value, Failure> {
        let target = with_query(url, query, client);
        let path = url.path().to_owned();
        let mut request = self
            .http
            .request(method.clone(), target)
            .header(header::ACCEPT, "application/json")
            .header(header::AUTHORIZATION, format!("OAuth {token}"));
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request.send().await.map_err(Error::unreachable)?;
        let status = response.status().as_u16();
        let bytes = response.bytes().await.map_err(Error::unreachable)?;
        if status == UNAUTHORIZED {
            return Err(Failure::Unauthorized(format!("{path} answered {status}")));
        }
        if !(200..300).contains(&status) {
            return Err(Error::from_status(status, format!("{path} answered {status}")).into());
        }
        if bytes.iter().all(u8::is_ascii_whitespace) {
            return Ok(Value::Null);
        }
        let answer: Value = serde_json::from_slice(&bytes)
            .map_err(|_| Error::Unreadable(format!("{path} answered with no JSON")))?;
        self.links.absorb(&answer);
        Ok(answer)
    }
}

impl std::fmt::Debug for Api {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Api")
            .field("session", &self.session)
            .finish_non_exhaustive()
    }
}

fn address(path: &str) -> Result<Url> {
    Url::parse(API)
        .and_then(|api| api.join(path))
        .map_err(|error| Error::Unreadable(error.to_string()))
}

fn with_query(url: &Url, query: &[(&str, String)], client: &str) -> Url {
    let retained: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(name, _)| name != CLIENT_ID)
        .map(|(name, value)| (name.into_owned(), value.into_owned()))
        .collect();
    let mut target = url.clone();
    target
        .query_pairs_mut()
        .clear()
        .extend_pairs(retained)
        .extend_pairs(query.iter().map(|(name, value)| (*name, value.as_str())))
        .append_pair(CLIENT_ID, client);
    target
}
