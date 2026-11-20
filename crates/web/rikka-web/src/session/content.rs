use std::sync::Arc;

use rikka_gecko_protocol::rikka::user_content::{
    self as wire, Frames, InjectAt, SetUserContent, StyleLevel,
};

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum World {
    #[default]
    Page,
    Isolated(String),
}

impl World {
    pub fn isolated(name: impl Into<String>) -> World {
        World::Isolated(name.into())
    }

    pub(crate) fn wire(&self) -> Option<String> {
        match self {
            World::Page => None,
            World::Isolated(name) => Some(name.clone()),
        }
    }

    pub(crate) fn from_name(world: Option<String>) -> World {
        world.map_or(World::Page, World::Isolated)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Matching {
    frames: Option<Frames>,
    allow: Vec<String>,
    block: Vec<String>,
}

macro_rules! matching {
    ($owner:ident) => {
        impl $owner {
            pub fn frames(mut self, frames: Frames) -> $owner {
                self.matching.frames = Some(frames);
                self
            }

            pub fn allow(mut self, pattern: impl Into<String>) -> $owner {
                self.matching.allow.push(pattern.into());
                self
            }

            pub fn block(mut self, pattern: impl Into<String>) -> $owner {
                self.matching.block.push(pattern.into());
                self
            }

            #[must_use]
            pub fn id(&self) -> &str {
                &self.id
            }
        }
    };
}

#[must_use]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserScript {
    id: String,
    source: Arc<str>,
    at: InjectAt,
    world: World,
    matching: Matching,
}

impl UserScript {
    pub fn new(id: impl Into<String>, source: impl Into<Arc<str>>) -> UserScript {
        UserScript {
            id: id.into(),
            source: source.into(),
            at: InjectAt::Start,
            world: World::Page,
            matching: Matching::default(),
        }
    }

    pub fn inject_at(mut self, at: InjectAt) -> UserScript {
        self.at = at;
        self
    }

    pub fn world(mut self, world: World) -> UserScript {
        self.world = world;
        self
    }

    fn wire(&self) -> wire::UserScript {
        wire::UserScript {
            id: self.id.clone(),
            source: self.source.as_ref().to_owned(),
            inject_at: self.at,
            frames: self.matching.frames.unwrap_or(Frames::All),
            world: self.world.wire(),
            allow: self.matching.allow.clone(),
            block: self.matching.block.clone(),
        }
    }
}

matching!(UserScript);

#[must_use]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserStyle {
    id: String,
    source: Arc<str>,
    level: StyleLevel,
    matching: Matching,
}

impl UserStyle {
    pub fn new(id: impl Into<String>, source: impl Into<Arc<str>>) -> UserStyle {
        UserStyle {
            id: id.into(),
            source: source.into(),
            level: StyleLevel::User,
            matching: Matching::default(),
        }
    }

    pub fn level(mut self, level: StyleLevel) -> UserStyle {
        self.level = level;
        self
    }

    fn wire(&self) -> wire::UserStyle {
        wire::UserStyle {
            id: self.id.clone(),
            source: self.source.as_ref().to_owned(),
            frames: self.matching.frames.unwrap_or(Frames::All),
            level: self.level,
            allow: self.matching.allow.clone(),
            block: self.matching.block.clone(),
        }
    }
}

matching!(UserStyle);

#[must_use]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageHandler {
    name: String,
    world: World,
    replies: bool,
}

impl MessageHandler {
    pub fn new(name: impl Into<String>) -> MessageHandler {
        MessageHandler {
            name: name.into(),
            world: World::Page,
            replies: false,
        }
    }

    pub fn world(mut self, world: World) -> MessageHandler {
        self.world = world;
        self
    }

    pub fn with_replies(mut self) -> MessageHandler {
        self.replies = true;
        self
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    fn wire(&self) -> wire::MessageHandler {
        wire::MessageHandler {
            name: self.name.clone(),
            world: self.world.wire(),
            replying: self.replies,
        }
    }
}

#[must_use]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UserContent {
    scripts: Vec<UserScript>,
    styles: Vec<UserStyle>,
    handlers: Vec<MessageHandler>,
}

impl UserContent {
    pub fn new() -> UserContent {
        UserContent::default()
    }

    pub fn script(mut self, script: UserScript) -> UserContent {
        self.scripts.retain(|other| other.id != script.id);
        self.scripts.push(script);
        self
    }

    pub fn style(mut self, style: UserStyle) -> UserContent {
        self.styles.retain(|other| other.id != style.id);
        self.styles.push(style);
        self
    }

    pub fn handler(mut self, handler: MessageHandler) -> UserContent {
        self.handlers
            .retain(|other| (&other.name, &other.world) != (&handler.name, &handler.world));
        self.handlers.push(handler);
        self
    }

    pub fn without(mut self, id: &str) -> UserContent {
        self.scripts.retain(|script| script.id != id);
        self.styles.retain(|style| style.id != id);
        self
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.scripts.is_empty() && self.styles.is_empty() && self.handlers.is_empty()
    }

    pub(crate) fn wire(&self) -> SetUserContent {
        SetUserContent {
            scripts: self.scripts.iter().map(UserScript::wire).collect(),
            styles: self.styles.iter().map(UserStyle::wire).collect(),
            handlers: self.handlers.iter().map(MessageHandler::wire).collect(),
        }
    }
}
