use rikka_net::{HeaderMap, HeaderName, HeaderValue, RequestBuilder, Url, header};
use serde_json::{Map, Value};

use crate::authorization::{Authorization, AuthorizationBuilder};
use crate::device::DeviceRequest;
use crate::error::{Error, Result};
use crate::grant::Grant;
use crate::http;

#[derive(Clone)]
pub struct Client {
    http: rikka_net::Client,
    id: String,
    secret: Option<String>,
    secret_placement: SecretPlacement,
    token_endpoint: Url,
    json: bool,
    headers: HeaderMap,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SecretPlacement {
    Body,
    BasicHeader,
}

#[must_use]
pub struct ClientBuilder {
    id: String,
    secret: Option<String>,
    secret_placement: SecretPlacement,
    token_endpoint: String,
    json: bool,
    headers: HeaderMap,
    error: Option<Error>,
}

impl Client {
    pub fn new(id: impl Into<String>, token_endpoint: impl Into<String>) -> Result<Client> {
        Client::builder(id, token_endpoint).build()
    }

    pub fn builder(id: impl Into<String>, token_endpoint: impl Into<String>) -> ClientBuilder {
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));

        ClientBuilder {
            id: id.into(),
            secret: None,
            secret_placement: SecretPlacement::Body,
            token_endpoint: token_endpoint.into(),
            json: false,
            headers,
            error: None,
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn http(&self) -> &rikka_net::Client {
        &self.http
    }

    pub fn authorization(
        &self,
        endpoint: impl Into<String>,
        redirect_uri: impl Into<String>,
    ) -> AuthorizationBuilder<'_> {
        AuthorizationBuilder::new(self, endpoint.into(), redirect_uri.into())
    }

    pub fn exchange(&self, code: &str, authorization: &Authorization) -> Grant<'_> {
        let mut grant = Grant::new(self, "authorization_code", None)
            .field("code", code)
            .field("redirect_uri", authorization.redirect_uri());
        if let Some(verifier) = authorization.verifier() {
            grant = grant.field("code_verifier", verifier);
        }
        grant
    }

    pub fn refresh(&self, refresh_token: &str) -> Grant<'_> {
        Grant::new(self, "refresh_token", Some(refresh_token.to_owned()))
            .field("refresh_token", refresh_token)
    }

    pub fn device(&self, endpoint: impl Into<String>) -> DeviceRequest<'_> {
        DeviceRequest::new(self, endpoint.into())
    }

    pub(crate) fn token_endpoint(&self) -> &Url {
        &self.token_endpoint
    }

    pub(crate) fn post(
        &self,
        url: Url,
        fields: &[(&str, String)],
        headers: HeaderMap,
    ) -> RequestBuilder {
        let mut identified: Vec<(&str, &str)> = vec![("client_id", self.id.as_str())];
        let mut request = self
            .http
            .post(url)
            .headers(self.headers.clone())
            .headers(headers);
        match (&self.secret, self.secret_placement) {
            (Some(secret), SecretPlacement::Body) => identified.push(("client_secret", secret)),
            (Some(secret), SecretPlacement::BasicHeader) => {
                request = request.basic_auth(form_encoded(&self.id), form_encoded(secret));
            }
            (None, _) => {}
        }
        identified.extend(fields.iter().map(|(name, value)| (*name, value.as_str())));

        match self.json {
            true => request.json(&Value::Object(
                identified
                    .into_iter()
                    .map(|(name, value)| (name.to_owned(), Value::String(value.to_owned())))
                    .collect::<Map<_, _>>(),
            )),
            false => request.form(&identified),
        }
    }
}

impl ClientBuilder {
    pub fn secret(mut self, secret: impl Into<String>) -> ClientBuilder {
        self.secret = Some(secret.into());
        self
    }

    pub fn basic(mut self) -> ClientBuilder {
        self.secret_placement = SecretPlacement::BasicHeader;
        self
    }

    pub fn json(mut self) -> ClientBuilder {
        self.json = true;
        self
    }

    pub fn header<K, V>(mut self, name: K, value: V) -> ClientBuilder
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
                    .get_or_insert(Error::InvalidHeader("client header"));
            }
        }
        self
    }

    pub fn build(self) -> Result<Client> {
        if let Some(error) = self.error {
            return Err(error);
        }

        Ok(Client {
            http: http::client()?,
            token_endpoint: endpoint(&self.token_endpoint)?,
            id: self.id,
            secret: self.secret,
            secret_placement: self.secret_placement,
            json: self.json,
            headers: self.headers,
        })
    }
}

pub(crate) fn endpoint(text: &str) -> Result<Url> {
    Url::parse(text.trim()).map_err(|_| Error::InvalidUrl(text.to_owned()))
}

fn form_encoded(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' => {
                encoded.push(char::from(byte));
            }
            b' ' => encoded.push('+'),
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    encoded
}

impl std::fmt::Debug for Client {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Client")
            .field("id", &self.id)
            .field("token_endpoint", &self.token_endpoint.as_str())
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for ClientBuilder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ClientBuilder")
            .field("id", &self.id)
            .field("token_endpoint", &self.token_endpoint)
            .finish_non_exhaustive()
    }
}
