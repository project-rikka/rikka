use std::time::Duration;

use rikka_net::{HeaderMap, HeaderValue, RequestBuilder, StatusCode, Url, header};
use serde_json::Value;

use crate::error::{Error, ErrorResponse, Result};

const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) fn client() -> Result<rikka_net::Client> {
    Ok(rikka_net::Client::builder()
        .read_timeout(TIMEOUT)
        .connect_timeout(TIMEOUT)
        .cookies(false)
        .build()?)
}

pub(crate) fn document(client: &rikka_net::Client, location: Url) -> RequestBuilder {
    client
        .get(location)
        .header(header::ACCEPT, "application/json")
}

pub(crate) async fn found(request: RequestBuilder) -> Result<Option<Value>> {
    let (status, body) = received(request).await?;
    match status {
        status if status.is_success() => Ok(Some(body).filter(Value::is_object)),
        status if status.is_client_error() => Ok(None),
        status => Err(Error::ServerResponse(ErrorResponse::from_response(
            status, &body,
        ))),
    }
}
pub async fn send(request: RequestBuilder) -> Result<Value> {
    let (status, body) = received(request).await?;
    match status.is_success() {
        true => Ok(body),
        false => Err(Error::ServerResponse(ErrorResponse::from_response(
            status, &body,
        ))),
    }
}

pub(crate) async fn received(request: RequestBuilder) -> Result<(StatusCode, Value)> {
    let response = request.send().await?;
    let status = response.status();
    let body = response.text().await?;

    let body = serde_json::from_str(&body).unwrap_or(Value::String(body));
    Ok((status, body))
}

pub fn bearer(access_token: &str) -> Result<HeaderMap> {
    let mut value = HeaderValue::from_str(&format!("Bearer {access_token}"))
        .map_err(|_| Error::InvalidHeader("access token"))?;
    value.set_sensitive(true);

    let mut headers = HeaderMap::new();
    headers.insert(header::AUTHORIZATION, value);
    Ok(headers)
}
