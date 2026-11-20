use std::collections::HashMap;

use crate::user::{User, deduplicated};

pub const SCREENCAST: &str = "rikka-screencast";
pub const CAST_APP: &str = "rikka.cast.app";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    AudioCapture,
    VideoCapture,
    VideoSource,
}

impl Kind {
    pub(crate) fn from_media_class(media_class: &str) -> Option<Kind> {
        match media_class {
            "Stream/Input/Audio" => Some(Kind::AudioCapture),
            "Stream/Input/Video" => Some(Kind::VideoCapture),
            "Video/Source" => Some(Kind::VideoSource),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Props(pub(crate) HashMap<String, String>);

impl Props {
    fn get(&self, key: &str) -> Option<&str> {
        self.0
            .get(key)
            .map(String::as_str)
            .filter(|value| !value.is_empty())
    }

    fn says(&self, key: &str) -> bool {
        self.get(key)
            .is_some_and(|value| value == "true" || value == "1")
    }

    fn user(&self) -> User {
        let binary = self.get("application.process.binary");
        User {
            name: self
                .get("application.name")
                .or(binary)
                .or_else(|| self.get("node.description"))
                .or_else(|| self.get("node.name"))
                .unwrap_or_default()
                .to_owned(),
            app_id: self.get("application.id").map(str::to_owned),
            pid: self
                .get("application.process.id")
                .and_then(|pid| pid.parse().ok()),
            icon: self.get("application.icon-name").map(str::to_owned),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Node {
    pub(crate) kind: Kind,
    pub(crate) running: bool,
    pub(crate) props: Props,
}

impl Node {
    pub(crate) fn informed(&mut self, props: Option<Props>, running: bool) {
        if let Some(props) = props {
            self.props = props;
        }
        self.running = running;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Link {
    pub(crate) output: u32,
    pub(crate) input: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Graph {
    pub(crate) nodes: HashMap<u32, Node>,
    pub(crate) links: HashMap<u32, Link>,
    pub(crate) clients: HashMap<u32, Props>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Users {
    pub(crate) microphone: Vec<User>,
    pub(crate) camera: Vec<User>,
    pub(crate) screen: Vec<User>,
}

impl Graph {
    pub(crate) fn users(&self) -> Users {
        Users {
            microphone: deduplicated(
                self.nodes
                    .values()
                    .filter(|node| node.kind == Kind::AudioCapture && node.running)
                    .filter(|node| {
                        !node.props.says("stream.monitor")
                            && !node.props.says("stream.capture.sink")
                    })
                    .map(|node| self.user(node))
                    .collect(),
            ),
            camera: self.consumers(|source| {
                source.running
                    && matches!(source.props.get("device.api"), Some("v4l2" | "libcamera"))
            }),
            screen: self.consumers(|source| source.props.get("node.name") == Some(SCREENCAST)),
        }
    }

    fn consumers(&self, sourced: impl Fn(&Node) -> bool) -> Vec<User> {
        let users = self.links.values().filter_map(|link| {
            let source = self.nodes.get(&link.output)?;
            if source.kind != Kind::VideoSource || !sourced(source) {
                return None;
            }
            let consumer = self.nodes.get(&link.input).map(|node| self.user(node));
            match consumer {
                Some(user) if !user.name.is_empty() => Some(user),
                _ => Some(User {
                    name: source.props.get(CAST_APP).unwrap_or_default().to_owned(),
                    app_id: source.props.get(CAST_APP).map(str::to_owned),
                    pid: None,
                    icon: None,
                }),
            }
        });
        deduplicated(users.collect())
    }

    fn user(&self, node: &Node) -> User {
        let mut user = node.props.user();
        let client = node
            .props
            .get("client.id")
            .and_then(|id| id.parse::<u32>().ok())
            .and_then(|id| self.clients.get(&id));
        if let Some(client) = client {
            user.pid = user.pid.or_else(|| {
                client
                    .get("application.process.id")
                    .or_else(|| client.get("pipewire.sec.pid"))
                    .and_then(|pid| pid.parse().ok())
            });
            if user.name.is_empty() {
                user.name = client
                    .get("application.name")
                    .unwrap_or_default()
                    .to_owned();
            }
        }
        user
    }
}
