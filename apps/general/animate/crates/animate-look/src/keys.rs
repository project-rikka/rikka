use std::collections::BTreeMap;
use std::fmt;

use animate_track::{Keyframe, Sampled as Interpolated, Tracks};
use rikka_keyframe::Framed;
use rikka_mmd::VmdCurve;
use rikka_shader_graph::{NodeId, Scope, SocketRef, SocketValue};
use serde::{Deserialize, Serialize};

use crate::graph_ref::{GraphRef, SocketKey};
use crate::name::Name;
use crate::value::LookValue;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SlotKey {
    pub figure: Name,
    pub material: u32,
    pub parameter: Name,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    Socket(SocketKey),
    Slot(SlotKey),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LookGroup {
    Node {
        graph: GraphRef,
        scope: Scope,
        node: NodeId,
    },
    Slot {
        figure: Name,
        material: u32,
    },
}

impl Target {
    #[must_use]
    pub fn group(&self) -> LookGroup {
        match self {
            Self::Socket(key) => LookGroup::Node {
                graph: key.graph.clone(),
                scope: key.socket.scope.clone(),
                node: key.socket.node,
            },
            Self::Slot(key) => LookGroup::Slot {
                figure: key.figure,
                material: key.material,
            },
        }
    }

    #[must_use]
    pub fn parameter(&self) -> &str {
        match self {
            Self::Socket(key) => &key.socket.port,
            Self::Slot(key) => key.parameter.as_str(),
        }
    }

    #[must_use]
    pub fn graph(&self) -> Option<&GraphRef> {
        match self {
            Self::Socket(key) => Some(&key.graph),
            Self::Slot(_) => None,
        }
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Socket(key) => key.fmt(f),
            Self::Slot(key) => write!(
                f,
                "{} material {} {}",
                key.figure, key.material, key.parameter
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LookKey {
    pub target: Target,
    pub frame: u32,
    pub value: LookValue,
    pub curve: VmdCurve,
}

impl Framed for LookKey {
    type Frame = u32;

    fn frame(&self) -> u32 {
        self.frame
    }
}

impl Keyframe for LookKey {
    type Id = Target;

    fn id(&self) -> &Target {
        &self.target
    }

    fn at_frame(self, frame: u32) -> Self {
        Self { frame, ..self }
    }
}

impl Interpolated for LookKey {
    type Value = LookValue;

    fn value(&self) -> LookValue {
        self.value.clone()
    }

    fn toward(&self, to: &Self, t: f32) -> LookValue {
        self.value.mix(&to.value, to.curve.factor(t))
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct KeyEdit {
    pub removed: Vec<LookKey>,
    pub added: Vec<LookKey>,
}

impl KeyEdit {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.removed.is_empty() && self.added.is_empty()
    }

    #[must_use]
    pub fn inverse(self) -> Self {
        Self {
            removed: self.added,
            added: self.removed,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Keys {
    tracks: Tracks<LookKey>,
}

impl Keys {
    #[must_use]
    pub fn tracks(&self) -> &Tracks<LookKey> {
        &self.tracks
    }

    pub fn tracks_mut(&mut self) -> &mut Tracks<LookKey> {
        &mut self.tracks
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    #[must_use]
    pub fn last_frame(&self) -> u32 {
        self.tracks.last_frame().unwrap_or(0)
    }

    #[must_use]
    pub fn value_at(&self, target: &Target, frame: f32) -> Option<LookValue> {
        self.tracks.value_at(target, frame)
    }

    pub fn apply(&mut self, edit: KeyEdit) -> KeyEdit {
        for key in &edit.removed {
            self.tracks.remove(&key.target, key.frame);
        }
        for key in &edit.added {
            self.tracks.insert(key.clone());
        }
        edit.inverse()
    }

    #[must_use]
    pub fn inserting(&self, key: LookKey) -> KeyEdit {
        KeyEdit {
            removed: self
                .tracks
                .key(&key.target, key.frame)
                .cloned()
                .into_iter()
                .collect(),
            added: vec![key],
        }
    }

    #[must_use]
    pub fn removing(&self, target: &Target, frame: u32) -> KeyEdit {
        KeyEdit {
            removed: self
                .tracks
                .key(target, frame)
                .cloned()
                .into_iter()
                .collect(),
            added: Vec::new(),
        }
    }

    #[must_use]
    pub fn moving(&self, target: &Target, from: u32, to: u32) -> KeyEdit {
        let Some(key) = self.tracks.key(target, from) else {
            return KeyEdit::default();
        };
        if from == to {
            return KeyEdit::default();
        }
        let mut removed = vec![key.clone()];
        removed.extend(self.tracks.key(target, to).cloned());
        KeyEdit {
            removed,
            added: vec![key.clone().at_frame(to)],
        }
    }

    #[must_use]
    pub fn retargeting(&self, from: &Target, to: &Target) -> KeyEdit {
        if from == to {
            return KeyEdit::default();
        }
        let moving = self.tracks.keys(from);
        let mut removed: Vec<LookKey> = moving.to_vec();
        removed.extend(self.tracks.keys(to).iter().cloned());
        KeyEdit {
            removed,
            added: moving
                .iter()
                .map(|key| LookKey {
                    target: to.clone(),
                    ..key.clone()
                })
                .collect(),
        }
    }

    #[must_use]
    pub fn dropping(&self, mut gone: impl FnMut(&Target) -> bool) -> KeyEdit {
        KeyEdit {
            removed: self
                .tracks
                .tracks()
                .filter(|(target, _)| gone(target))
                .flat_map(|(_, keys)| keys.iter().cloned())
                .collect(),
            added: Vec::new(),
        }
    }

    #[must_use]
    pub fn renaming_material(&self, from: &str, to: &str) -> KeyEdit {
        let mut edit = KeyEdit::default();
        for (target, keys) in self.tracks.tracks() {
            let Target::Socket(key) = target else {
                continue;
            };
            if key.graph != GraphRef::Material(from.to_owned()) {
                continue;
            }
            let renamed = Target::Socket(SocketKey::new(
                GraphRef::Material(to.to_owned()),
                key.socket.clone(),
            ));
            edit.removed.extend(keys.iter().cloned());
            edit.added.extend(keys.iter().map(|key| LookKey {
                target: renamed.clone(),
                ..key.clone()
            }));
        }
        edit
    }

    #[must_use]
    pub fn sample(&self, frame: f32) -> Sampled {
        let mut sampled = Sampled::default();
        for (target, _) in self.tracks.tracks() {
            let Some(value) = self.tracks.value_at(target, frame) else {
                continue;
            };
            match target {
                Target::Socket(key) => {
                    if let Some(value) = value.to_socket() {
                        sampled
                            .sockets
                            .entry(key.graph.clone())
                            .or_default()
                            .insert(key.socket.clone(), value);
                    }
                }
                Target::Slot(key) => {
                    sampled.slots.insert(*key, value);
                }
            }
        }
        sampled
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sampled {
    sockets: BTreeMap<GraphRef, BTreeMap<SocketRef, SocketValue>>,
    slots: BTreeMap<SlotKey, LookValue>,
}

impl Sampled {
    #[must_use]
    pub fn socket(&self, key: &SocketKey) -> Option<&SocketValue> {
        self.socket_in(&key.graph, &key.socket)
    }

    #[must_use]
    pub fn socket_in(&self, graph: &GraphRef, socket: &SocketRef) -> Option<&SocketValue> {
        self.sockets.get(graph)?.get(socket)
    }

    #[must_use]
    pub fn sockets_of(&self, graph: &GraphRef) -> Option<&BTreeMap<SocketRef, SocketValue>> {
        self.sockets.get(graph)
    }

    #[must_use]
    pub fn slot(&self, key: &SlotKey) -> Option<&LookValue> {
        self.slots.get(key)
    }

    pub fn slot_values(
        &self,
        figure: Name,
        material: u32,
    ) -> impl Iterator<Item = (Name, &LookValue)> {
        self.slots
            .iter()
            .filter(move |(key, _)| key.figure == figure && key.material == material)
            .map(|(key, value)| (key.parameter, value))
    }
}
