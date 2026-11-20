use std::pin::Pin;
use std::task::{Context, Poll};

use futures_util::StreamExt;

use crate::error::{Error, Result};
use crate::event::Event;
use crate::response::Response;

type Source = Pin<Box<dyn futures_util::Stream<Item = Result<Event>> + Send>>;

pub struct Stream {
    source: Source,
}

impl Stream {
    pub(crate) fn new(
        source: impl futures_util::Stream<Item = Result<Event>> + Send + 'static,
    ) -> Stream {
        Stream {
            source: Box::pin(source),
        }
    }

    pub async fn next(&mut self) -> Option<Result<Event>> {
        self.source.next().await
    }

    pub async fn response(mut self) -> Result<Response> {
        while let Some(event) = self.next().await {
            if let Event::Done(response) = event? {
                return Ok(*response);
            }
        }

        Err(Error::Malformed(
            "the stream ended before the answer did".to_owned(),
        ))
    }

    pub async fn text(self) -> Result<String> {
        Ok(self.response().await?.text())
    }
}

impl futures_util::Stream for Stream {
    type Item = Result<Event>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Event>>> {
        self.source.as_mut().poll_next(context)
    }
}

impl std::fmt::Debug for Stream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Stream").finish_non_exhaustive()
    }
}
