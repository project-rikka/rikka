use std::future::Future;
use std::pin::Pin;

use rikka_tool::Reply;

use crate::order::Order;

pub type ToolResponse = Pin<Box<dyn Future<Output = Result<Reply, String>> + Send>>;

pub trait ToolHost: Send + Sync + 'static {
    fn handle(&self, order: Order) -> ToolResponse;
}
