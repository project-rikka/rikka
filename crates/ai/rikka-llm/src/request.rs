use std::time::Duration;

use rikka_types::CancelToken;

use crate::cache::{Cache, Ttl};
use crate::effort::{Effort, Reveal};
use crate::format::Format;
use crate::message::Message;
use crate::server_tool::ServerTool;
use crate::text::Text;
use crate::tool_choice::ToolChoice;
use rikka_tool::Tool;

#[derive(Debug, Clone)]
pub struct Request {
    pub(crate) model: Option<String>,
    pub(crate) system: Vec<Text>,
    pub(crate) messages: Vec<Message>,
    pub(crate) tools: Vec<Tool>,
    pub(crate) server_tools: Vec<ServerTool>,
    pub(crate) tool_choice: ToolChoice,
    pub(crate) parallel_tools: bool,
    pub(crate) max_tokens: Option<u32>,
    pub(crate) temperature: Option<f32>,
    pub(crate) top_p: Option<f32>,
    pub(crate) stop: Vec<String>,
    pub(crate) effort: Option<Effort>,
    pub(crate) reveal: Reveal,
    pub(crate) format: Option<Format>,
    pub(crate) cache: Cache,
    pub(crate) ttl: Ttl,
    pub(crate) cache_key: Option<String>,
    pub(crate) cancel: Option<CancelToken>,
    pub(crate) timeout: Option<Duration>,
}

impl Request {
    #[must_use]
    pub fn system(&self) -> &[Text] {
        &self.system
    }

    #[must_use]
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    #[must_use]
    pub fn tools(&self) -> &[Tool] {
        &self.tools
    }

    #[must_use]
    pub fn server_tools(&self) -> &[ServerTool] {
        &self.server_tools
    }

    pub fn push(&mut self, message: Message) {
        self.messages.push(message);
    }

    pub fn extend(&mut self, messages: impl IntoIterator<Item = Message>) {
        self.messages.extend(messages);
    }

    pub fn set_messages(&mut self, messages: Vec<Message>) {
        self.messages = messages;
    }

    pub fn set_tools(&mut self, tools: Vec<Tool>) {
        self.tools = tools;
    }

    #[must_use]
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    #[must_use]
    pub fn cache(&self) -> (Cache, Ttl, Option<&str>) {
        (self.cache, self.ttl, self.cache_key.as_deref())
    }

    #[must_use]
    pub fn ttl(&self) -> Ttl {
        self.ttl
    }

    #[must_use]
    pub fn cache_key(&self) -> Option<&str> {
        self.cache_key.as_deref()
    }

    #[must_use]
    pub fn effort(&self) -> Option<Effort> {
        self.effort
    }

    #[must_use]
    pub fn reveal(&self) -> Reveal {
        self.reveal
    }

    #[must_use]
    pub fn format(&self) -> Option<&Format> {
        self.format.as_ref()
    }
}

impl Default for Request {
    fn default() -> Request {
        Request {
            model: None,
            system: Vec::new(),
            messages: Vec::new(),
            tools: Vec::new(),
            server_tools: Vec::new(),
            tool_choice: ToolChoice::Auto,
            parallel_tools: true,
            max_tokens: None,
            temperature: None,
            top_p: None,
            stop: Vec::new(),
            effort: None,
            reveal: Reveal::Hidden,
            format: None,
            cache: Cache::Off,
            ttl: Ttl::Short,
            cache_key: None,
            cancel: None,
            timeout: None,
        }
    }
}
