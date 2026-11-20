use rikka_net::{HeaderMap, HeaderName, HeaderValue};

use crate::client::Client;
use crate::error::{Error, Result};
use crate::http;
use crate::token::Token;

#[must_use]
pub struct Grant<'a> {
    client: &'a Client,
    fields: Vec<(&'static str, String)>,
    headers: HeaderMap,
    refreshing: Option<String>,
    error: Option<Error>,
}

impl<'a> Grant<'a> {
    pub(crate) fn new(client: &'a Client, kind: &str, refreshing: Option<String>) -> Grant<'a> {
        Grant {
            client,
            fields: vec![("grant_type", kind.to_owned())],
            headers: HeaderMap::new(),
            refreshing,
            error: None,
        }
    }

    pub fn field(mut self, name: &'static str, value: impl Into<String>) -> Grant<'a> {
        self.fields.push((name, value.into()));
        self
    }

    pub fn header<K, V>(mut self, name: K, value: V) -> Grant<'a>
    where
        K: TryInto<HeaderName>,
        V: TryInto<HeaderValue>,
    {
        match (name.try_into(), value.try_into()) {
            (Ok(name), Ok(value)) => {
                self.headers.insert(name, value);
            }
            _ => {
                self.error
                    .get_or_insert(Error::InvalidHeader("grant header"));
            }
        }
        self
    }

    pub async fn send(self) -> Result<Token> {
        if let Some(error) = self.error {
            return Err(error);
        }

        let request = self.client.post(
            self.client.token_endpoint().clone(),
            &self.fields,
            self.headers,
        );
        let token = Token::from_response(http::send(request).await?)?;

        Ok(match &self.refreshing {
            Some(refresh_token) => token.or_refresh_token(refresh_token),
            None => token,
        })
    }
}

impl std::fmt::Debug for Grant<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Grant")
            .field("client", self.client)
            .finish_non_exhaustive()
    }
}
