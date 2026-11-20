use std::future::Future;
use std::pin::Pin;

use serde_json::Value;

use crate::reply::Reply;

pub type Answer = Pin<Box<dyn Future<Output = Result<Reply, String>> + Send>>;

pub trait Handler: Send + Sync + 'static {
    fn call(&self, input: Value) -> Answer;
}

impl<F, Fut, T, E> Handler for F
where
    F: Fn(Value) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<T, E>> + Send + 'static,
    T: Into<Reply> + Send + 'static,
    E: std::fmt::Display + Send + 'static,
{
    fn call(&self, input: Value) -> Answer {
        let answer = self(input);
        Box::pin(async move {
            match answer.await {
                Ok(reply) => Ok(reply.into()),
                Err(error) => Err(error.to_string()),
            }
        })
    }
}
