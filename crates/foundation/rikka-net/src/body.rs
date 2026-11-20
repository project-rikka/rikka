use std::fmt;
use std::path::Path;
use std::pin::Pin;

use bytes::{Bytes, BytesMut};
use futures_util::{Stream, StreamExt, stream};
use serde::Serialize;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::error::{Error, Result};
use crate::reporter::Reporter;

const CHUNK: usize = 64 * 1024;

pub(crate) type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes>> + Send + 'static>>;

pub struct Body {
    payload: Payload,
    length: Option<u64>,
}

enum Payload {
    Bytes(Bytes),
    Stream(ByteStream),
}

impl Body {
    #[must_use]
    pub fn empty() -> Body {
        Body::from(Bytes::new())
    }

    #[must_use]
    pub fn bytes(bytes: impl Into<Bytes>) -> Body {
        Body::from(bytes.into())
    }

    #[must_use]
    pub fn text(text: impl Into<String>) -> Body {
        Body::from(text.into())
    }

    pub fn json<T: Serialize + ?Sized>(value: &T) -> Result<Body> {
        let bytes = serde_json::to_vec(value).map_err(|error| Error::Encode {
            format: "JSON",
            source: Box::new(error),
        })?;
        Ok(Body::from(bytes))
    }

    pub async fn file(path: impl AsRef<Path>) -> Result<Body> {
        let file = tokio::fs::File::open(path).await?;
        let length = file.metadata().await?.len();

        Ok(Body {
            payload: Payload::Stream(Box::pin(read(file))),
            length: Some(length),
        })
    }

    #[must_use]
    pub fn reader(reader: impl AsyncRead + Send + Unpin + 'static, length: Option<u64>) -> Body {
        Body {
            payload: Payload::Stream(Box::pin(read(reader))),
            length,
        }
    }

    #[must_use]
    pub fn stream<S, B, E>(stream: S, length: Option<u64>) -> Body
    where
        S: Stream<Item = std::result::Result<B, E>> + Send + 'static,
        B: Into<Bytes>,
        E: Into<crate::error::Cause>,
    {
        let stream = stream.map(|chunk| match chunk {
            Ok(bytes) => Ok(bytes.into()),
            Err(error) => Err(Error::Body(error.into())),
        });

        Body {
            payload: Payload::Stream(Box::pin(stream)),
            length,
        }
    }

    #[must_use]
    pub fn length(&self) -> Option<u64> {
        self.length
    }

    pub(crate) fn from_stream(stream: ByteStream, length: Option<u64>) -> Body {
        Body {
            payload: Payload::Stream(stream),
            length,
        }
    }

    pub(crate) fn metered(self, reporter: Reporter) -> Body {
        let length = self.length;
        let stream = stream::try_unfold(
            (self.into_stream(), reporter),
            |(mut stream, mut reporter)| async move {
                match stream.next().await.transpose()? {
                    Some(chunk) => {
                        reporter.advance(chunk.len() as u64)?;
                        Ok(Some((chunk, (stream, reporter))))
                    }
                    None => {
                        reporter.report()?;
                        Ok(None)
                    }
                }
            },
        );

        Body {
            payload: Payload::Stream(Box::pin(stream)),
            length,
        }
    }

    pub(crate) fn into_stream(self) -> ByteStream {
        match self.payload {
            Payload::Bytes(bytes) => Box::pin(stream::once(async move { Ok(bytes) })),
            Payload::Stream(stream) => stream,
        }
    }

    pub(crate) fn into_parts(self) -> (reqwest::Body, Option<u64>) {
        match self.payload {
            Payload::Bytes(bytes) => (reqwest::Body::from(bytes), None),
            Payload::Stream(stream) => (reqwest::Body::wrap_stream(stream), self.length),
        }
    }
}

fn read<R: AsyncRead + Send + Unpin + 'static>(
    reader: R,
) -> impl Stream<Item = Result<Bytes>> + Send + 'static {
    stream::try_unfold(
        (reader, BytesMut::with_capacity(CHUNK)),
        |(mut reader, mut buffer)| async move {
            if buffer.capacity() < CHUNK / 4 {
                buffer.reserve(CHUNK);
            }

            match reader.read_buf(&mut buffer).await? {
                0 => Ok(None),
                read => Ok(Some((buffer.split_to(read).freeze(), (reader, buffer)))),
            }
        },
    )
}

impl From<Bytes> for Body {
    fn from(bytes: Bytes) -> Body {
        Body {
            length: Some(bytes.len() as u64),
            payload: Payload::Bytes(bytes),
        }
    }
}

impl From<Vec<u8>> for Body {
    fn from(bytes: Vec<u8>) -> Body {
        Body::from(Bytes::from(bytes))
    }
}

impl From<&[u8]> for Body {
    fn from(bytes: &[u8]) -> Body {
        Body::from(Bytes::copy_from_slice(bytes))
    }
}

impl From<String> for Body {
    fn from(text: String) -> Body {
        Body::from(Bytes::from(text))
    }
}

impl From<&str> for Body {
    fn from(text: &str) -> Body {
        Body::from(Bytes::copy_from_slice(text.as_bytes()))
    }
}

impl fmt::Debug for Body {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.payload {
            Payload::Bytes(_) => "bytes",
            Payload::Stream(_) => "stream",
        };
        f.debug_struct("Body")
            .field("kind", &kind)
            .field("length", &self.length)
            .finish()
    }
}
