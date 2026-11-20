use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_channel::oneshot;
use serde::de::DeserializeOwned;

use crate::error::{Error, Result};
use crate::wire::calls::Outcome;

#[must_use]
pub(crate) struct Asked<T> {
    state: State,
    answer: PhantomData<fn() -> T>,
}

enum State {
    Waiting(oneshot::Receiver<Outcome>),
    Failed(Option<Error>),
}

impl<T> Asked<T> {
    pub(crate) fn waiting(receiver: oneshot::Receiver<Outcome>) -> Asked<T> {
        Asked {
            state: State::Waiting(receiver),
            answer: PhantomData,
        }
    }

    pub(crate) fn failed(error: Error) -> Asked<T> {
        Asked {
            state: State::Failed(Some(error)),
            answer: PhantomData,
        }
    }
}

impl<T: DeserializeOwned> Future for Asked<T> {
    type Output = Result<T>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Result<T>> {
        let this = self.get_mut();
        match &mut this.state {
            State::Failed(error) => Poll::Ready(Err(error.take().unwrap_or(Error::EngineExited))),
            State::Waiting(receiver) => match Pin::new(receiver).poll(context) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(oneshot::Canceled)) => Poll::Ready(Err(Error::EngineExited)),
                Poll::Ready(Ok(Err(error))) => Poll::Ready(Err(error)),
                Poll::Ready(Ok(Ok(value))) => Poll::Ready(
                    T::deserialize(value).map_err(|error| Error::Malformed(error.to_string())),
                ),
            },
        }
    }
}
