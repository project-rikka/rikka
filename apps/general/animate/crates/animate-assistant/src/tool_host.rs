use std::future::Future;
use std::pin::Pin;

use rikka_tool::Reply;

use crate::request::ToolRequest;
use crate::tool_error::ToolError;

pub type ToolResponse = Pin<Box<dyn Future<Output = Result<Reply, ToolError>> + Send>>;

pub trait ToolHost: Send + Sync + 'static {
    fn handle(&self, request: ToolRequest) -> ToolResponse;
}
