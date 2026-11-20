use std::io::Write;
use std::net::SocketAddr;

use bytes::Bytes;
use futures_util::StreamExt;
use http::{HeaderMap, StatusCode, Version};
use serde::de::DeserializeOwned;
use url::Url;

use crate::error::Result;
use crate::response::Response;
use crate::runtime;

#[derive(Debug)]
pub struct BlockingResponse {
    inner: Response,
}

impl BlockingResponse {
    pub(crate) fn new(inner: Response) -> BlockingResponse {
        BlockingResponse { inner }
    }

    #[must_use]
    pub fn status(&self) -> StatusCode {
        self.inner.status()
    }

    #[must_use]
    pub fn is_success(&self) -> bool {
        self.inner.is_success()
    }

    #[must_use]
    pub fn headers(&self) -> &HeaderMap {
        self.inner.headers()
    }

    #[must_use]
    pub fn url(&self) -> &Url {
        self.inner.url()
    }

    #[must_use]
    pub fn version(&self) -> Version {
        self.inner.version()
    }

    #[must_use]
    pub fn content_length(&self) -> Option<u64> {
        self.inner.content_length()
    }

    #[must_use]
    pub fn content_type(&self) -> Option<&str> {
        self.inner.content_type()
    }

    #[must_use]
    pub fn remote_addr(&self) -> Option<SocketAddr> {
        self.inner.remote_addr()
    }

    pub fn error_for_status(self) -> Result<BlockingResponse> {
        runtime::block_on_local(self.inner.error_for_status()).map(BlockingResponse::new)
    }

    pub fn bytes(self) -> Result<Bytes> {
        runtime::block_on_local(self.inner.bytes())
    }

    pub fn bytes_limited(self, limit: usize) -> Result<Bytes> {
        runtime::block_on_local(self.inner.bytes_limited(limit))
    }

    pub fn text(self) -> Result<String> {
        runtime::block_on_local(self.inner.text())
    }

    pub fn json<T: DeserializeOwned>(self) -> Result<T> {
        runtime::block_on_local(self.inner.json())
    }

    pub fn copy_to<W: Write>(self, writer: &mut W) -> Result<u64> {
        runtime::block_on_local(async move {
            let mut stream = Box::pin(self.inner.bytes_stream());
            let mut written = 0u64;

            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                writer.write_all(&chunk)?;
                written += chunk.len() as u64;
            }

            writer.flush()?;
            Ok(written)
        })
    }
}
