use std::time::Duration;

use rikka_types::CancelToken;

use crate::cache::{Cache, Ttl};
use crate::content::IntoContent;
use crate::effort::{Effort, Reveal};
use crate::error::Result;
use crate::format::Format;
use crate::message::Message;
use crate::model::Model;
use crate::request::Request;
use crate::response::Response;
use crate::server_tool::ServerTool;
use crate::stream::Stream;
use crate::text::Text;
use crate::tool_choice::ToolChoice;
use rikka_tool::Tool;

#[must_use]
#[derive(Debug, Clone)]
pub struct RequestBuilder {
    model: Model,
    request: Request,
}

impl RequestBuilder {
    pub(crate) fn new(model: Model) -> RequestBuilder {
        RequestBuilder {
            model,
            request: Request::default(),
        }
    }

    pub fn system(mut self, system: impl Into<Text>) -> Self {
        self.request.system.push(system.into());
        self
    }

    pub fn user(mut self, content: impl IntoContent) -> Self {
        self.request.messages.push(Message::user(content));
        self
    }

    pub fn assistant(mut self, content: impl IntoContent) -> Self {
        self.request.messages.push(Message::assistant(content));
        self
    }

    pub fn message(mut self, message: Message) -> Self {
        self.request.messages.push(message);
        self
    }

    pub fn messages(mut self, messages: impl IntoIterator<Item = Message>) -> Self {
        self.request.messages.extend(messages);
        self
    }

    pub fn tool(mut self, tool: Tool) -> Self {
        self.request.tools.push(tool);
        self
    }

    pub fn tools(mut self, tools: impl IntoIterator<Item = Tool>) -> Self {
        self.request.tools.extend(tools);
        self
    }

    pub fn server_tools(mut self, tools: impl IntoIterator<Item = ServerTool>) -> Self {
        self.request.server_tools.extend(tools);
        self.request.server_tools.sort_unstable();
        self.request.server_tools.dedup();
        self
    }

    pub fn tool_choice(mut self, choice: ToolChoice) -> Self {
        self.request.tool_choice = choice;
        self
    }

    pub fn parallel_tools(mut self, parallel: bool) -> Self {
        self.request.parallel_tools = parallel;
        self
    }

    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.request.max_tokens = Some(tokens);
        self
    }

    pub fn temperature(mut self, temperature: f32) -> Self {
        self.request.temperature = Some(temperature);
        self
    }

    pub fn top_p(mut self, top_p: f32) -> Self {
        self.request.top_p = Some(top_p);
        self
    }

    pub fn stop(mut self, sequence: impl Into<String>) -> Self {
        self.request.stop.push(sequence.into());
        self
    }

    pub fn effort(mut self, effort: Effort) -> Self {
        self.request.effort = Some(effort);
        self
    }

    pub fn reveal(mut self, reveal: Reveal) -> Self {
        self.request.reveal = reveal;
        self
    }

    pub fn format(mut self, format: Format) -> Self {
        self.request.format = Some(format);
        self
    }

    pub fn cache(mut self, cache: Cache) -> Self {
        self.request.cache = cache;
        self
    }

    pub fn ttl(mut self, ttl: Ttl) -> Self {
        self.request.ttl = ttl;
        self
    }

    pub fn cache_key(mut self, key: impl Into<String>) -> Self {
        self.request.cache_key = Some(key.into());
        self
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.request.model = Some(model.into());
        self
    }

    pub fn cancel_token(mut self, token: CancelToken) -> Self {
        self.request.cancel = Some(token);
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.request.timeout = Some(timeout);
        self
    }

    pub fn build(self) -> Request {
        self.request
    }

    pub async fn send(self) -> Result<Response> {
        self.model.send(&self.request).await
    }

    pub async fn stream(self) -> Result<Stream> {
        self.model.stream(&self.request).await
    }
}
