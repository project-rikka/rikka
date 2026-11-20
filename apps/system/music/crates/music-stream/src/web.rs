use std::time::Duration;

use music_core::{Error, Result};
use rikka_net::{Bytes, Client, RequestBuilder, Response, header};

const PATIENCE: Duration = Duration::from_secs(20);

#[must_use]
pub fn api_client(agent: &str) -> Client {
    Client::builder()
        .user_agent(agent)
        .timeout(PATIENCE)
        .build()
        .unwrap_or_default()
}

pub async fn answer(request: RequestBuilder) -> Result<Response> {
    let response = request.send().await.map_err(Error::unreachable)?;
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let host = response.url().host_str().unwrap_or("the host");
    Err(Error::from_status(
        status.as_u16(),
        format!("{host} answered {}", status.as_u16()),
    ))
}

pub async fn answer_bytes(request: RequestBuilder) -> Result<Bytes> {
    answer(request)
        .await?
        .bytes()
        .await
        .map_err(Error::unreachable)
}

pub async fn answer_text(request: RequestBuilder) -> Result<String> {
    answer(request)
        .await?
        .text()
        .await
        .map_err(Error::unreachable)
}

pub async fn answer_length(request: RequestBuilder) -> Result<u64> {
    answer(request)
        .await?
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|length| length.to_str().ok())
        .and_then(|length| length.parse().ok())
        .filter(|length| *length > 0)
        .ok_or_else(|| Error::Unreadable("the host reported no length".into()))
}
