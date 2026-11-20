use rikka_sync::{Condvar, Mutex, MutexGuard};
use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use serde_json::{Value, json};

use crate::asked::Asked;
use crate::guard::Guard;
use crate::reply::{Reply, event};

const ENDPOINT: &str = "/mcp";
const SESSION_ID: &str = "mcp-session-id";
const LAST_EVENT_ID: &str = "last-event-id";
const REVISION: &str = "2025-11-25";
const RECONNECTION_MILLISECONDS: u32 = 10;
const METHOD_NOT_FOUND: i64 = -32601;
const TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answering {
    Json,
    Stream,
    Interrupted,
}

#[derive(Clone)]
pub struct Remote {
    base: String,
    shared: Arc<Shared>,
}

struct Shared {
    base: String,
    answering: Answering,
    state: Mutex<State>,
    arrived: Condvar,
}

#[derive(Default)]
struct State {
    asked: Vec<Asked>,
    guard: Option<Guard>,
    session: Option<String>,
    sessions: u32,
    tools: Vec<String>,
    listeners: Vec<mpsc::Sender<String>>,
    unheard: Vec<String>,
    held: HashMap<String, String>,
    events: u32,
}

impl Remote {
    #[must_use]
    pub fn start(answering: Answering) -> Remote {
        Remote::serving(answering, None)
    }

    #[must_use]
    pub fn guarded() -> Remote {
        Remote::serving(Answering::Json, Some(Guard::default()))
    }

    fn serving(answering: Answering, guard: Option<Guard>) -> Remote {
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("test server should bind a free port");
        let base = format!(
            "http://{}",
            listener
                .local_addr()
                .expect("listener should have a local address")
        );
        let shared = Arc::new(Shared {
            base: base.clone(),
            answering,
            state: Mutex::new(State {
                guard,
                tools: vec![String::from("say")],
                ..State::default()
            }),
            arrived: Condvar::new(),
        });

        let serving = Arc::clone(&shared);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else {
                    return;
                };
                let serving = Arc::clone(&serving);
                std::thread::spawn(move || serving.serve(stream));
            }
        });
        Remote { base, shared }
    }

    #[must_use]
    pub fn url(&self) -> String {
        format!("{}{ENDPOINT}", self.base)
    }

    #[must_use]
    pub fn asked(&self) -> Vec<Asked> {
        self.shared.state().asked.clone()
    }

    #[must_use]
    pub fn rpc_methods(&self) -> Vec<String> {
        self.asked().iter().filter_map(Asked::rpc_method).collect()
    }

    #[must_use]
    pub fn expect(&self, wanted: impl Fn(&Asked) -> bool) -> Asked {
        let (state, _) =
            self.shared
                .arrived
                .wait_timeout_while(self.shared.state(), TIMEOUT, |state| {
                    !state.asked.iter().any(&wanted)
                });
        state
            .asked
            .iter()
            .find(|asked| wanted(asked))
            .cloned()
            .expect("the awaited request should arrive")
    }

    pub fn offer(&self, tool: &str) {
        let mut state = self.shared.state();
        state.tools.push(tool.to_owned());
        let changed = state.event(&json!({
            "jsonrpc": "2.0",
            "method": "notifications/tools/list_changed",
        }));
        state
            .listeners
            .retain(|listener| listener.send(changed.clone()).is_ok());
        if state.listeners.is_empty() {
            state.unheard.push(changed);
        }
    }

    pub fn expire_session(&self) {
        let mut state = self.shared.state();
        state.session = None;
        state.listeners.clear();
    }
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock()
    }

    fn serve(&self, mut stream: TcpStream) {
        let Some(asked) = Asked::read(&stream) else {
            return;
        };
        let reply = self.reply(&asked);
        let _ = reply.write(&mut stream);
    }

    fn reply(&self, asked: &Asked) -> Reply {
        let mut state = self.state();
        state.asked.push(asked.clone());
        self.arrived.notify_all();

        if asked.path() != ENDPOINT {
            let page = state
                .guard
                .as_mut()
                .and_then(|guard| guard.page(&self.base, asked));
            return page.unwrap_or_else(|| Reply::status("404 Not Found"));
        }
        if state
            .guard
            .as_ref()
            .is_some_and(|guard| !guard.admits(asked))
        {
            return Guard::challenge(&self.base);
        }

        match asked.method() {
            "POST" => self.posted(&mut state, asked),
            "GET" if state.is_attached(asked) => state.listened(asked),
            "DELETE" if state.is_attached(asked) => {
                state.session = None;
                state.listeners.clear();
                Reply::status("200 OK")
            }
            "GET" | "DELETE" => Reply::status("404 Not Found"),
            _ => Reply::status("405 Method Not Allowed"),
        }
    }

    fn posted(&self, state: &mut State, asked: &Asked) -> Reply {
        let Some(message) = asked.json() else {
            return Reply::status("400 Bad Request");
        };
        let method = message.get("method").and_then(Value::as_str);
        let id = message.get("id");

        if method == Some("initialize") {
            state.sessions += 1;
            let session = format!("session-{}", state.sessions);
            state.session = Some(session.clone());
            let greeting = answer(
                id,
                Ok(json!({
                    "protocolVersion": REVISION,
                    "capabilities": { "tools": { "listChanged": true } },
                    "serverInfo": { "name": "remote", "title": "Remote", "version": "1.0" },
                })),
            );
            return Reply::json("200 OK", greeting).with_header(SESSION_ID, session);
        }
        if !state.is_attached(asked) {
            return Reply::status("404 Not Found");
        }
        let (Some(method), Some(id)) = (method, id) else {
            return Reply::status("202 Accepted");
        };

        let answer = answer(Some(id), state.result(method, &message));
        match self.answering {
            Answering::Json => Reply::json("200 OK", answer),
            Answering::Stream => {
                let ping = json!({ "jsonrpc": "2.0", "id": "are-you-there", "method": "ping" });
                Reply::Events(vec![state.event(&ping), state.event(&answer)])
            }
            Answering::Interrupted if method == "tools/call" => {
                state.events += 1;
                let id = format!("event-{}", state.events);
                state.held.insert(id.clone(), event(&id, &answer));
                Reply::Events(vec![format!(
                    "retry: {RECONNECTION_MILLISECONDS}\nid: {id}\ndata:\n\n"
                )])
            }
            Answering::Interrupted => Reply::json("200 OK", answer),
        }
    }
}

impl State {
    fn is_attached(&self, asked: &Asked) -> bool {
        self.session.is_some() && asked.header(SESSION_ID) == self.session.as_deref()
    }

    fn event(&mut self, message: &Value) -> String {
        self.events += 1;
        event(&format!("event-{}", self.events), message)
    }

    fn listened(&mut self, asked: &Asked) -> Reply {
        let held = asked
            .header(LAST_EVENT_ID)
            .and_then(|last| self.held.remove(last));
        if let Some(held) = held {
            return Reply::Events(vec![held]);
        }

        let (listener, events) = mpsc::channel();
        for unheard in self.unheard.drain(..) {
            let _ = listener.send(unheard);
        }
        self.listeners.push(listener);
        Reply::Listening(events)
    }

    fn result(&self, method: &str, message: &Value) -> Result<Value, Value> {
        match method {
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({
                "tools": self
                    .tools
                    .iter()
                    .map(|tool| json!({
                        "name": tool,
                        "description": format!("Does {tool}"),
                        "inputSchema": { "type": "object" },
                    }))
                    .collect::<Vec<Value>>(),
            })),
            "tools/call" => {
                let tool = message["params"]["name"].as_str().unwrap_or_default();
                Ok(json!({ "content": [{ "type": "text", "text": format!("{tool} done") }] }))
            }
            unknown => Err(json!({
                "code": METHOD_NOT_FOUND,
                "message": format!("method not found: {unknown}"),
            })),
        }
    }
}

fn answer(id: Option<&Value>, outcome: Result<Value, Value>) -> Value {
    match outcome {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
    }
}

impl std::fmt::Debug for Remote {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Remote")
            .field("base", &self.base)
            .finish_non_exhaustive()
    }
}
