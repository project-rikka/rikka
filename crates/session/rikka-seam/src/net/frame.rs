use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;

use crate::capability::{CapabilityId, Version};
use crate::event::Event;
use crate::payload::Payload;

pub(crate) const MAX_FRAME: usize = 1 << 25;

const HEADER: usize = 4;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "t", rename_all = "kebab-case")]
pub(crate) enum Frame {
    Call {
        id: u64,
        capability: CapabilityId,
        method: String,
        payload: Payload,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        require: Option<Version>,
    },
    Reply {
        id: u64,
        payload: Payload,
    },
    Fail {
        id: u64,
        code: u16,
        message: String,
    },
    Subscribe {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        capability: Option<CapabilityId>,
    },
    Event {
        event: Event,
    },
    #[serde(rename = "missed")]
    Lagged {
        capabilities: Vec<CapabilityId>,
        count: u64,
    },
}

pub(crate) async fn write<W: AsyncWrite + Unpin>(sink: &mut W, frame: &Frame) -> io::Result<()> {
    let mut encoded = vec![0u8; HEADER];
    serde_json::to_writer(&mut encoded, frame).map_err(io::Error::other)?;

    let body = encoded.len() - HEADER;
    let length = u32::try_from(body)
        .ok()
        .filter(|length| *length as usize <= MAX_FRAME)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("a seam frame of {body} bytes is too large to send"),
            )
        })?;
    encoded[..HEADER].copy_from_slice(&length.to_le_bytes());

    sink.write_all(&encoded).await?;
    sink.flush().await
}

pub(crate) const OUTGOING: usize = 256;

pub(crate) fn writer<W: AsyncWrite + Unpin + Send + 'static>(
    mut sink: W,
    mut queued: mpsc::Receiver<Frame>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async move {
        while let Some(frame) = queued.recv().await {
            if let Err(error) = write(&mut sink, &frame).await {
                tracing::debug!("failed to write to seam peer: {error}");
                break;
            }
        }
        let _ = sink.shutdown().await;
    })
}

pub(crate) async fn read<R: AsyncRead + Unpin>(source: &mut R) -> io::Result<Option<Frame>> {
    let mut header = [0u8; HEADER];
    match source.read_exact(&mut header).await {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error),
    }

    let length = u32::from_le_bytes(header) as usize;
    if length > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("a seam frame claimed {length} bytes"),
        ));
    }

    let mut body = vec![0u8; length];
    source.read_exact(&mut body).await?;
    serde_json::from_slice(&body).map(Some).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("a seam frame could not be read: {error}"),
        )
    })
}
