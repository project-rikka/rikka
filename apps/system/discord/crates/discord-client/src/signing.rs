use std::sync::Arc;

use discord_core::Picture;
use futures_channel::oneshot;

use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Method {
    QrCode,
    Browser,
}

impl Method {
    pub const ALL: [Method; 2] = [Method::QrCode, Method::Browser];
}

#[derive(Debug)]
pub enum SignInStep {
    Qr(QrStep),
    Web(WebSignIn),
}

pub type Prompt = Arc<dyn Fn(SignInStep) + Send + Sync>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QrStep {
    Code(String),
    Scanned {
        name: String,
        avatar: Option<Picture>,
    },
}

pub struct WebSignIn {
    url: &'static str,
    script: &'static str,
    message: &'static str,
    reply: oneshot::Sender<String>,
}

impl WebSignIn {
    pub(crate) fn new(
        url: &'static str,
        script: &'static str,
        message: &'static str,
    ) -> (WebSignIn, oneshot::Receiver<String>) {
        let (reply, answer) = oneshot::channel();
        let request = WebSignIn {
            url,
            script,
            message,
            reply,
        };
        (request, answer)
    }

    #[must_use]
    pub fn url(&self) -> &'static str {
        self.url
    }

    #[must_use]
    pub fn script(&self) -> &'static str {
        self.script
    }

    #[must_use]
    pub fn message(&self) -> &'static str {
        self.message
    }

    pub fn answer(self, posted: String) {
        let _ = self.reply.send(posted);
    }
}

impl std::fmt::Debug for WebSignIn {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WebSignIn")
            .field("url", &self.url)
            .field("message", &self.message)
            .finish_non_exhaustive()
    }
}

pub(crate) async fn answered(answer: oneshot::Receiver<String>) -> Result<String> {
    answer.await.map_err(|_| Error::Cancelled)
}
