use std::path::PathBuf;

use rikka_remote_wire::DecodeError;

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("bad certificate: {0}")]
    Certificate(String),
    #[error("cannot keep the identity at {path}: {error}", path = .path.display())]
    Identity {
        path: PathBuf,
        error: std::io::Error,
    },
    #[error("TLS setup failed: {0}")]
    Tls(#[from] rustls::Error),
    #[error("QUIC setup failed: {0}")]
    Quic(String),
    #[error("cannot open the endpoint: {0}")]
    Endpoint(std::io::Error),
    #[error("cannot connect: {0}")]
    Connect(#[from] quinn::ConnectError),
    #[error("the connection failed: {0}")]
    Connection(#[from] quinn::ConnectionError),
    #[error("a stream broke off: {0}")]
    Read(#[from] quinn::ReadExactError),
    #[error("a stream broke off: {0}")]
    ReadStream(#[from] quinn::ReadError),
    #[error("cannot write to a stream: {0}")]
    Write(#[from] quinn::WriteError),
    #[error("cannot send a datagram: {0}")]
    Datagram(#[from] quinn::SendDatagramError),
    #[error("malformed message: {0}")]
    Decode(#[from] DecodeError),
    #[error("the stream closed")]
    Closed,
}

impl LinkError {
    pub fn is_closed(&self) -> bool {
        let lost = match self {
            LinkError::Connection(error) => error,
            LinkError::Read(quinn::ReadExactError::ReadError(
                quinn::ReadError::ConnectionLost(error),
            ))
            | LinkError::ReadStream(quinn::ReadError::ConnectionLost(error))
            | LinkError::Write(quinn::WriteError::ConnectionLost(error)) => error,
            _ => return false,
        };
        matches!(
            lost,
            quinn::ConnectionError::ApplicationClosed(_) | quinn::ConnectionError::LocallyClosed
        )
    }
}
