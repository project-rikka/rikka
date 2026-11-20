use std::collections::HashMap;

use crate::node::NodeKind;

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Defaults {
    pub(super) sink: Option<String>,
    pub(super) source: Option<String>,
    targets: HashMap<u32, String>,
}

impl Defaults {
    pub(super) fn apply(&mut self, subject: u32, key: Option<&str>, value: Option<&str>) {
        match (subject, key) {
            (_, None) => self.forget(subject),
            (0, Some("default.audio.sink")) => self.sink = named(value),
            (0, Some("default.audio.source")) => self.source = named(value),
            (0, Some(_)) => {}
            (node, Some("target.object")) => self.aim(node, value.and_then(aimed)),
            (_, Some(_)) => {}
        }
    }

    pub(super) fn choose(&mut self, kind: NodeKind, name: &str) {
        match kind {
            NodeKind::Sink => self.sink = Some(name.to_owned()),
            NodeKind::Source => self.source = Some(name.to_owned()),
            NodeKind::Playback | NodeKind::Recording => {}
        }
    }

    pub(super) fn aim(&mut self, node: u32, target: Option<String>) {
        match target {
            Some(target) => {
                self.targets.insert(node, target);
            }
            None => {
                self.targets.remove(&node);
            }
        }
    }

    pub(super) fn forget(&mut self, subject: u32) {
        if subject == 0 {
            self.sink = None;
            self.source = None;
        } else {
            self.targets.remove(&subject);
        }
    }

    pub(super) fn target(&self, node: u32) -> Option<&str> {
        self.targets.get(&node).map(String::as_str)
    }
}

fn aimed(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value != "-1").then(|| value.to_owned())
}

fn named(value: Option<&str>) -> Option<String> {
    let after = value?.split_once("\"name\"")?.1;
    let quoted = after.trim_start().strip_prefix(':')?.trim_start();
    let quoted = quoted.strip_prefix('"')?;
    let (name, _) = quoted.split_once('"')?;
    Some(name.to_owned())
}
