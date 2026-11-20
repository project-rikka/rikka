use std::io;

use serde::Serialize;
use serde_json::{Map, Value};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::agent::features::AgentFeatures;
use crate::agent::status::{ErrorMessage, StatusMessage};

const MAX_MESSAGE_LEN: usize = 1 << 20;

pub(crate) enum Inbound {
    Status(Box<StatusMessage>),
    Error(ErrorMessage),
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
enum Outbound<'a> {
    FeaturesSet(&'a AgentFeatures),
    StatusGet(StatusGet),
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct StatusGet {
    features_statistics: bool,
}

pub(crate) async fn read<R: AsyncRead + Unpin>(reader: &mut R) -> io::Result<Vec<u8>> {
    let length = reader.read_u32().await?;
    let length = usize::try_from(length).unwrap_or(usize::MAX);
    if length > MAX_MESSAGE_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "local agent message exceeds 1 MiB",
        ));
    }
    let mut message = vec![0_u8; length];
    reader.read_exact(&mut message).await?;
    Ok(message)
}

pub(crate) async fn send_features<W: AsyncWrite + Unpin>(
    writer: &mut W,
    features: &AgentFeatures,
) -> io::Result<()> {
    write(writer, &Outbound::FeaturesSet(features)).await
}

pub(crate) async fn request_status<W: AsyncWrite + Unpin>(
    writer: &mut W,
    statistics: bool,
) -> io::Result<()> {
    write(
        writer,
        &Outbound::StatusGet(StatusGet {
            features_statistics: statistics,
        }),
    )
    .await
}

pub(crate) fn parse(message: &[u8]) -> io::Result<Vec<Inbound>> {
    let parsed: Map<String, Value> = serde_json::from_slice(message)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    Ok(parsed
        .into_iter()
        .filter_map(|(kind, value)| match kind.as_str() {
            "status" => match serde_json::from_value(value) {
                Ok(status) => Some(Inbound::Status(Box::new(status))),
                Err(error) => {
                    tracing::warn!(%error, "failed to parse local agent status");
                    None
                }
            },
            "error" => match serde_json::from_value(value) {
                Ok(error) => Some(Inbound::Error(error)),
                Err(error) => {
                    tracing::warn!(%error, "failed to parse local agent error");
                    None
                }
            },
            other => {
                tracing::debug!(kind = other, "unknown local agent message");
                None
            }
        })
        .collect())
}

async fn write<W: AsyncWrite + Unpin>(writer: &mut W, message: &Outbound<'_>) -> io::Result<()> {
    let body = serde_json::to_vec(message).map_err(io::Error::other)?;
    let length = u32::try_from(body.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "message too long"))?;
    let mut frame = Vec::with_capacity(body.len() + 4);
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(&body);
    writer.write_all(&frame).await?;
    writer.flush().await
}
