use futures_util::future::BoxFuture;
use rikka_net::HeaderMap;
use tokio::process::{ChildStderr, Command};
use tokio::sync::mpsc;

use crate::client::http_link::HttpLink;
use crate::client::stdio_link::StdioLink;
use crate::credentials::{Signing, SigningError};
use crate::protocol::{Message, RequestId};

const ARRIVALS_QUEUED: usize = 64;

#[derive(Debug)]
pub(crate) enum Link {
    Stdio(StdioLink),
    Http(HttpLink),
}

#[derive(Debug)]
pub(crate) enum Arrival {
    Message(Message),
    Lost { request: RequestId, reason: String },
}

pub(crate) type Arrivals = mpsc::Receiver<Arrival>;

#[derive(Debug, thiserror::Error)]
pub(crate) enum LinkError {
    #[error("failed to write to the server")]
    Pipe(#[source] std::io::Error),

    #[error(transparent)]
    Http(#[from] rikka_net::Error),

    #[error("the server wants a sign-in")]
    Unauthorized { challenge: Option<String> },

    #[error(transparent)]
    Signing(#[from] SigningError),

    #[error("the server no longer knows this session")]
    SessionExpired,

    #[error("the server answered with {0}, which is neither JSON nor an event stream")]
    ContentType(String),

    #[error("the server answered with something that is no message")]
    Malformed(#[source] serde_json::Error),

    #[error("failed to encode the message")]
    Encode(#[source] serde_json::Error),
}

impl Link {
    pub(crate) fn stdio(
        command: Command,
    ) -> std::io::Result<(Link, Arrivals, Option<ChildStderr>)> {
        let (arriving, arrivals) = mpsc::channel(ARRIVALS_QUEUED);
        let (link, stderr) = StdioLink::spawn(command, arriving)?;
        Ok((Link::Stdio(link), arrivals, stderr))
    }

    pub(crate) fn http(
        url: &str,
        headers: HeaderMap,
        signing: Option<Signing>,
    ) -> Result<(Link, Arrivals), LinkError> {
        let (arriving, arrivals) = mpsc::channel(ARRIVALS_QUEUED);
        let link = HttpLink::new(url, headers, signing, arriving)?;
        Ok((Link::Http(link), arrivals))
    }

    pub(crate) fn send<'a>(&'a self, message: &'a Message) -> BoxFuture<'a, Result<(), LinkError>> {
        match self {
            Link::Stdio(link) => Box::pin(link.send(message)),
            Link::Http(link) => Box::pin(link.send(message)),
        }
    }

    pub(crate) fn negotiated(&self, revision: &str) {
        match self {
            Link::Stdio(_) => {}
            Link::Http(link) => link.negotiated(revision),
        }
    }

    pub(crate) fn listen(&self) {
        match self {
            Link::Stdio(_) => {}
            Link::Http(link) => link.listen(),
        }
    }
}
