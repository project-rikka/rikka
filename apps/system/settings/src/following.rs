use std::fmt::Display;
use std::future::Future;

use futures_util::{Stream, StreamExt};

pub(crate) fn following<T, S, E>(
    opening: impl Future<Output = Result<S, E>> + Send + 'static,
) -> impl Stream<Item = Result<T, String>> + Send + 'static
where
    T: Send + 'static,
    S: Stream<Item = T> + Send + 'static,
    E: Display + Send + 'static,
{
    futures_util::stream::once(opening).flat_map(|opened| match opened {
        Ok(items) => items.map(Ok).boxed(),
        Err(error) => futures_util::stream::once(async move { Err(error.to_string()) }).boxed(),
    })
}
