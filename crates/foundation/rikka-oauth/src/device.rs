use std::time::Duration;

use serde_json::Value;

use crate::client::{Client, endpoint};
use crate::error::{Error, ErrorResponse, Result, text};
use crate::http;
use crate::token::Token;

const GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
const INTERVAL: Duration = Duration::from_secs(5);
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(1800);

#[must_use]
pub struct DeviceRequest<'a> {
    client: &'a Client,
    endpoint: String,
    scope: Option<String>,
}

pub struct DeviceAuthorization {
    client: Client,
    device_code: String,
    user_code: String,
    verification_url: String,
    interval: Duration,
    expires_in: u64,
}

impl<'a> DeviceRequest<'a> {
    pub(crate) fn new(client: &'a Client, endpoint: String) -> DeviceRequest<'a> {
        DeviceRequest {
            client,
            endpoint,
            scope: None,
        }
    }

    pub fn scope(mut self, scope: impl Into<String>) -> DeviceRequest<'a> {
        self.scope = Some(scope.into());
        self
    }

    pub async fn send(self) -> Result<DeviceAuthorization> {
        let fields: Vec<_> = self
            .scope
            .into_iter()
            .map(|scope| ("scope", scope))
            .collect();
        let request = self
            .client
            .post(endpoint(&self.endpoint)?, &fields, Default::default());

        DeviceAuthorization::from_response(self.client.clone(), &http::send(request).await?)
    }
}

impl DeviceAuthorization {
    fn from_response(client: Client, body: &Value) -> Result<DeviceAuthorization> {
        let device_code = text(body, "device_code");
        let user_code = text(body, "user_code");
        let verification_url = match text(body, "verification_uri_complete") {
            url if url.is_empty() => text(body, "verification_uri"),
            url => url,
        };
        if device_code.is_empty() || user_code.is_empty() || verification_url.is_empty() {
            return Err(Error::MissingField("user_code"));
        }

        let count = |field| body.get(field).and_then(Value::as_u64).unwrap_or_default();
        Ok(DeviceAuthorization {
            client,
            device_code,
            user_code,
            verification_url,
            interval: INTERVAL.max(Duration::from_secs(count("interval"))),
            expires_in: count("expires_in"),
        })
    }

    #[must_use]
    pub fn user_code(&self) -> &str {
        &self.user_code
    }

    #[must_use]
    pub fn verification_url(&self) -> &str {
        &self.verification_url
    }

    pub async fn token(self) -> Result<Token> {
        let patience = match self.expires_in {
            0 => CALLBACK_TIMEOUT,
            seconds => CALLBACK_TIMEOUT.min(Duration::from_secs(seconds)),
        };

        tokio::time::timeout(patience, self.poll())
            .await
            .map_err(|_| Error::TimedOut)?
    }

    async fn poll(mut self) -> Result<Token> {
        loop {
            let request = self.client.post(
                self.client.token_endpoint().clone(),
                &[
                    ("grant_type", GRANT.to_owned()),
                    ("device_code", self.device_code.clone()),
                ],
                Default::default(),
            );
            let (status, body) = http::received(request).await?;

            match text(&body, "error").as_str() {
                "authorization_pending" => {}
                "slow_down" => self.interval += INTERVAL,
                "access_denied" => return Err(Error::AccessDenied),
                "" if status.is_success() => return Token::from_response(body),
                _ => {
                    return Err(Error::ServerResponse(ErrorResponse::from_response(
                        status, &body,
                    )));
                }
            }

            tokio::time::sleep(self.interval).await;
        }
    }
}

impl std::fmt::Debug for DeviceRequest<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeviceRequest")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for DeviceAuthorization {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeviceAuthorization")
            .field("user_code", &self.user_code)
            .field("verification_url", &self.verification_url)
            .finish_non_exhaustive()
    }
}
