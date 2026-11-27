use std::sync::Arc;

use serde::Serialize;
use serde_json::{Value, json};

use crate::channel::Link;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum World {
    Isolated,
    Page,
}

impl World {
    pub(crate) fn chrome(self) -> &'static str {
        match self {
            World::Isolated => "ISOLATED",
            World::Page => "MAIN",
        }
    }

    pub(crate) fn parse(world: &str) -> World {
        match world {
            "page" => World::Page,
            _ => World::Isolated,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Location {
    Bundled(&'static str),
    Core(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Script(Location);

impl Script {
    #[must_use]
    pub const fn bundled(path: &'static str) -> Script {
        Script(Location::Bundled(path))
    }

    pub(crate) fn file(self) -> String {
        match self.0 {
            Location::Bundled(path) => format!("scripts/{path}"),
            Location::Core(file) => file.to_owned(),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Frames {
    #[default]
    Top,
    All,
}

#[must_use]
#[derive(Clone, Debug)]
pub struct Run {
    pub(crate) script: Option<Script>,
    pub(crate) function: Option<String>,
    pub(crate) args: Vec<Value>,
    pub(crate) world: World,
    pub(crate) frames: Frames,
}

impl Run {
    pub fn load(script: Script) -> Run {
        Run {
            script: Some(script),
            function: None,
            args: Vec::new(),
            world: World::Isolated,
            frames: Frames::Top,
        }
    }

    pub fn call(mut self, function: impl Into<String>) -> Run {
        self.function = Some(function.into());
        self
    }

    pub fn arg(mut self, value: impl Into<Value>) -> Run {
        self.args.push(value.into());
        self
    }

    pub fn world(mut self, world: World) -> Run {
        self.world = world;
        self
    }

    pub fn frames(mut self, frames: Frames) -> Run {
        self.frames = frames;
        self
    }

    pub(crate) fn request(&self, tab: i64) -> Value {
        let mut target = json!({ "tabId": tab });
        if self.frames == Frames::All {
            target["allFrames"] = Value::Bool(true);
        }
        json!({
            "target": target,
            "world": self.world.chrome(),
            "files": self.script.map(|script| vec![script.file()]).unwrap_or_default(),
            "invoke": self.function,
            "args": self.args,
        })
    }
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct FrameResult {
    pub frame: i64,
    pub value: Value,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum InjectAt {
    Start,
    #[default]
    End,
    Idle,
}

impl InjectAt {
    fn chrome(self) -> &'static str {
        match self {
            InjectAt::Start => "document_start",
            InjectAt::End => "document_end",
            InjectAt::Idle => "document_idle",
        }
    }
}

#[must_use]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserScript {
    pub(crate) id: String,
    pub(crate) script: Script,
    pub(crate) world: World,
    pub(crate) inject_at: InjectAt,
    pub(crate) frames: Frames,
    pub(crate) matches: Vec<String>,
}

impl UserScript {
    pub fn new(id: impl Into<String>, script: Script) -> UserScript {
        UserScript {
            id: id.into(),
            script,
            world: World::Isolated,
            inject_at: InjectAt::End,
            frames: Frames::Top,
            matches: vec!["<all_urls>".into()],
        }
    }

    pub fn world(mut self, world: World) -> UserScript {
        self.world = world;
        self
    }

    pub fn inject_at(mut self, inject_at: InjectAt) -> UserScript {
        self.inject_at = inject_at;
        self
    }

    pub fn frames(mut self, frames: Frames) -> UserScript {
        self.frames = frames;
        self
    }

    pub fn matches(mut self, patterns: impl IntoIterator<Item = impl Into<String>>) -> UserScript {
        self.matches = patterns.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn registration(&self) -> Value {
        Registration {
            id: &self.id,
            js: vec![self.script.file()],
            matches: &self.matches,
            run_at: self.inject_at.chrome(),
            world: self.world.chrome(),
            all_frames: self.frames == Frames::All,
            persist_across_sessions: false,
        }
        .value()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Registration<'a> {
    id: &'a str,
    js: Vec<String>,
    matches: &'a [String],
    run_at: &'a str,
    world: &'a str,
    all_frames: bool,
    persist_across_sessions: bool,
}

impl Registration<'_> {
    fn value(&self) -> Value {
        serde_json::to_value(self).expect("registrations always encode")
    }
}

pub(crate) const RELAY: Script = Script(Location::Core("relay.js"));

pub(crate) const RELAY_ID: &str = "rikka-relay";

#[derive(Clone, Debug, PartialEq)]
pub struct ScriptMessage {
    pub name: String,
    pub body: Value,
    pub world: World,
    pub frame: i64,
    pub url: Option<String>,
}

pub struct ScriptAsk {
    pub name: String,
    pub body: Value,
    pub frame: i64,
    pub url: Option<String>,
    ask: Option<u64>,
    link: Arc<Link>,
}

impl ScriptAsk {
    pub(crate) fn new(message: ScriptMessage, ask: u64, link: Arc<Link>) -> ScriptAsk {
        ScriptAsk {
            name: message.name,
            body: message.body,
            frame: message.frame,
            url: message.url,
            ask: Some(ask),
            link,
        }
    }

    pub fn respond(mut self, value: Value) {
        if let Some(ask) = self.ask.take() {
            self.link.answer(ask, &value);
        }
    }
}

impl Drop for ScriptAsk {
    fn drop(&mut self) {
        if let Some(ask) = self.ask.take() {
            self.link.answer(ask, &Value::Null);
        }
    }
}

impl std::fmt::Debug for ScriptAsk {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ScriptAsk")
            .field("name", &self.name)
            .field("frame", &self.frame)
            .finish_non_exhaustive()
    }
}
