use rikka_gecko_protocol::rikka::user_content::ScriptMessage as WireMessage;
use serde_json::Value;

use crate::session::content::World;
use crate::wire::Answer;

#[derive(Debug)]
pub struct ScriptMessage {
    pub name: String,
    pub world: World,
    pub body: Value,
    pub frame_uri: String,
    pub top_frame: bool,
    pub reply: Option<ScriptReply>,
}

impl ScriptMessage {
    pub(crate) fn from_wire(message: WireMessage, answer: Option<Answer>) -> ScriptMessage {
        ScriptMessage {
            name: message.name,
            world: World::from_name(message.world),
            body: message.body,
            frame_uri: message.frame.url,
            top_frame: message.frame.top,
            reply: answer.map(|answer| ScriptReply { answer }),
        }
    }
}

#[derive(Debug)]
pub struct ScriptReply {
    answer: Answer,
}

impl ScriptReply {
    pub fn send(self, value: Value) {
        self.answer.with(&value);
    }

    pub fn fail(self, message: impl Into<String>) {
        self.answer.refuse(message);
    }
}
