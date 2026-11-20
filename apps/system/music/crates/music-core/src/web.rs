use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_channel::oneshot;

use crate::error::{Error, Result};

#[derive(Clone, PartialEq, Eq)]
pub struct Cookie {
    pub name: String,
    pub value: String,
}

impl std::fmt::Debug for Cookie {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Cookie")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

pub struct WebSignIn {
    url: String,
    landing: &'static str,
    cookies_of: &'static str,
    proof: &'static [&'static str],
    reply: oneshot::Sender<Vec<Cookie>>,
}

#[derive(Debug)]
pub struct WebAnswer(oneshot::Receiver<Vec<Cookie>>);

impl WebSignIn {
    #[must_use]
    pub fn new(
        url: impl Into<String>,
        landing: &'static str,
        cookies_of: &'static str,
        proof: &'static [&'static str],
    ) -> (WebSignIn, WebAnswer) {
        let (reply, answer) = oneshot::channel();
        let request = WebSignIn {
            url: url.into(),
            landing,
            cookies_of,
            proof,
            reply,
        };
        (request, WebAnswer(answer))
    }

    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    #[must_use]
    pub fn cookies_of(&self) -> &str {
        self.cookies_of
    }

    #[must_use]
    pub fn arrived(&self, uri: &str) -> bool {
        host(uri) == Some(self.landing)
    }

    #[must_use]
    pub fn has_proof_cookie(&self, cookies: &[Cookie]) -> bool {
        cookies
            .iter()
            .any(|cookie| !cookie.value.is_empty() && self.proof.contains(&cookie.name.as_str()))
    }

    pub fn answer(self, cookies: Vec<Cookie>) {
        let _ = self.reply.send(cookies);
    }
}

impl std::fmt::Debug for WebSignIn {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WebSignIn")
            .field("url", &self.url)
            .field("landing", &self.landing)
            .finish_non_exhaustive()
    }
}

impl Future for WebAnswer {
    type Output = Result<Vec<Cookie>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.get_mut().0)
            .poll(cx)
            .map(|answered| answered.map_err(|_| Error::Cancelled))
    }
}

fn host(uri: &str) -> Option<&str> {
    let (_, rest) = uri.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?;
    Some(host.split(':').next().unwrap_or(host))
}
