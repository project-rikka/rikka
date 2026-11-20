use rikka_sync::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rikka_llm::{Content, Error, IntoContent, Model, Stop, ToolUse, Usage};
use serde_json::{Value, json};

use crate::asked::Asked;
use crate::frames;
use crate::server::Server;

const MODEL: &str = "scripted";

#[derive(Debug, Clone)]
pub struct Scripted {
    server: Server,
    script: Arc<Script>,
}

#[derive(Debug, Default)]
struct Script {
    answers: Mutex<VecDeque<Answer>>,
    usage: Mutex<Usage>,
    queued: AtomicUsize,
}

#[derive(Debug)]
enum Answer {
    Said(Value),
    Refused { status: u16, body: Value },
    Interrupted(Value),
}

impl Scripted {
    #[must_use]
    pub fn new() -> Scripted {
        let script = Arc::new(Script::default());
        let answering = Arc::clone(&script);
        let server = Server::answering(move |request| answering.answer(request));
        Scripted { server, script }
    }

    pub fn costing(&self, usage: Usage) -> &Self {
        *self.script.usage.lock() = usage;
        self
    }

    pub fn says(&self, content: impl IntoContent) -> &Self {
        self.answers(content, Stop::End)
    }

    pub fn asks(&self, name: &str, input: Value) -> &Self {
        let id = format!("call_{}", self.script.queued.load(Ordering::Relaxed) + 1);
        self.asks_all([ToolUse::new(id, name, input)])
    }

    pub fn asks_all(&self, calls: impl IntoIterator<Item = ToolUse>) -> &Self {
        let content: Vec<Content> = calls.into_iter().map(Content::ToolUse).collect();
        self.answers(content, Stop::ToolUse)
    }

    pub fn searches(
        &self,
        query: &str,
        urls: &[&str],
        content: impl IntoContent,
        stop: Stop,
    ) -> &Self {
        let id = format!(
            "srvtoolu_{}",
            self.script.queued.load(Ordering::Relaxed) + 1
        );
        let found: Vec<Value> = urls
            .iter()
            .map(|url| {
                json!({
                    "type": "web_search_result",
                    "url": url,
                    "title": url,
                    "encrypted_content": "scripted",
                })
            })
            .collect();
        let mut blocks = vec![
            json!({
                "type": "server_tool_use",
                "id": id,
                "name": "web_search",
                "input": { "query": query },
            }),
            json!({ "type": "web_search_tool_result", "tool_use_id": id, "content": found }),
        ];
        blocks.extend(content.into_content().iter().map(frames::block));
        self.respond(blocks, &stop)
    }

    pub fn answers(&self, content: impl IntoContent, stop: Stop) -> &Self {
        let blocks = content.into_content().iter().map(frames::block).collect();
        self.respond(blocks, &stop)
    }

    fn respond(&self, blocks: Vec<Value>, stop: &Stop) -> &Self {
        let id = format!(
            "msg_{}",
            self.script.queued.fetch_add(1, Ordering::Relaxed) + 1
        );
        let usage = *self.script.usage.lock();
        let response = frames::message(&id, blocks, stop, usage);
        self.queue(Answer::Said(response))
    }

    pub fn fails(&self, error: Error) -> &Self {
        let refused = |message: &str| Answer::Refused {
            status: 400,
            body: json!({
                "type": "error",
                "error": { "type": "invalid_request_error", "message": message },
            }),
        };
        let answer = match error {
            Error::ContextOverflow => refused("prompt is too long: 200001 tokens > 200000 maximum"),
            Error::Rejected(reason) => refused(&reason),
            Error::Overloaded => Answer::Interrupted(json!({
                "type": "error",
                "error": { "type": "overloaded_error", "message": "Overloaded" },
            })),
            other => panic!("{other} cannot be scripted over the wire"),
        };
        self.queue(answer)
    }

    #[must_use]
    pub fn model(&self) -> Model {
        self.server.messages(MODEL)
    }

    #[must_use]
    pub fn requests(&self) -> Vec<Asked> {
        self.server.requests().into_iter().map(Asked::new).collect()
    }

    #[must_use]
    pub fn remaining(&self) -> usize {
        self.script.answers.lock().len()
    }

    fn queue(&self, answer: Answer) -> &Self {
        self.script.answers.lock().push_back(answer);
        self
    }
}

impl Default for Scripted {
    fn default() -> Scripted {
        Scripted::new()
    }
}

impl Script {
    fn answer(&self, request: &Value) -> Vec<u8> {
        let streamed = request["stream"].as_bool().unwrap_or(false);
        let next = self.answers.lock().pop_front();
        match next {
            Some(Answer::Said(response)) if streamed => frames::events(frames::streamed(&response)),
            Some(Answer::Said(response)) => frames::json(200, &response),
            Some(Answer::Refused { status, body }) => frames::json(status, &body),
            Some(Answer::Interrupted(failure)) if streamed => frames::events([failure.to_string()]),
            Some(Answer::Interrupted(failure)) => frames::json(400, &failure),
            None => frames::json(
                400,
                &json!({
                    "type": "error",
                    "error": {
                        "type": "invalid_request_error",
                        "message": "the script ran out of answers",
                    },
                }),
            ),
        }
    }
}
