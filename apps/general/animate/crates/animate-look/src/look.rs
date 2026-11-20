use std::collections::BTreeMap;
use std::sync::Arc;

use animate_track::Tracks;
use rikka_mmd::VmdCurve;
use rikka_shader_graph::{Edit, Graph, Hint, Library, NodeId, Port, Scope, SocketRef, SocketValue};

use crate::asset::Asset;
use crate::defaults::{default_composite, default_world};
use crate::error::Error;
use crate::graph_ref::{GraphRef, SocketKey, check_material_key};
use crate::keys::{KeyEdit, Keys, LookKey, Target};
use crate::slots::{SlotEdit, Slots};
use crate::value::LookValue;
use crate::world::shared_library;

#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub world: Arc<Graph>,
    pub composite: Arc<Graph>,
    pub materials: BTreeMap<String, Arc<Graph>>,
    pub slots: Slots,
    pub keys: Keys,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LookEdit {
    Graph { graph: GraphRef, edit: Edit },
    Replace { graph: GraphRef, with: Arc<Graph> },
    InsertMaterial { key: String, graph: Arc<Graph> },
    RemoveMaterial { key: String },
    RenameMaterial { from: String, to: String },
    Slots(SlotEdit),
    Keys(KeyEdit),
    Batch(Vec<LookEdit>),
}

impl Default for Look {
    fn default() -> Self {
        Self::new(shared_library()).expect("the built-in look nodes build the default graphs")
    }
}

pub(crate) fn check_kind(graph: &GraphRef, held: &Graph) -> Result<(), Error> {
    let kind = graph.kind();
    match held.kind() == kind {
        true => Ok(()),
        false => Err(Error::Kind {
            graph: graph.clone(),
            expected: kind.name(),
            found: held.kind().name(),
        }),
    }
}

pub(crate) fn port_of(graph: &Graph, library: &Library, socket: &SocketRef) -> Option<Port> {
    let node = graph.scope(&socket.scope)?.node(socket.node)?;
    let ports = graph.ports(library, &socket.scope, &node.kind).ok()?;
    ports.input(&socket.port).cloned()
}

pub(crate) fn port_hint(graph: &Graph, library: &Library, socket: &SocketRef) -> Option<Hint> {
    port_of(graph, library, socket).map(|port| port.hint)
}

fn holds_input(graph: &Graph, library: &Library, socket: &SocketRef) -> bool {
    port_hint(graph, library, socket).is_some()
}

impl Look {
    pub fn new(library: &Library) -> Result<Self, Error> {
        Ok(Self {
            world: Arc::new(default_world(library)?),
            composite: Arc::new(default_composite(library)?),
            materials: BTreeMap::new(),
            slots: Slots::default(),
            keys: Keys::default(),
        })
    }

    #[must_use]
    pub fn graph(&self, graph: &GraphRef) -> Option<&Arc<Graph>> {
        match graph {
            GraphRef::World => Some(&self.world),
            GraphRef::Composite => Some(&self.composite),
            GraphRef::Material(key) => self.materials.get(key),
        }
    }

    pub fn graphs(&self) -> impl Iterator<Item = (GraphRef, &Arc<Graph>)> {
        [
            (GraphRef::World, &self.world),
            (GraphRef::Composite, &self.composite),
        ]
        .into_iter()
        .chain(
            self.materials
                .iter()
                .map(|(key, graph)| (GraphRef::Material(key.clone()), graph)),
        )
    }

    fn graph_mut(&mut self, graph: &GraphRef) -> Result<&mut Arc<Graph>, Error> {
        match graph {
            GraphRef::World => Ok(&mut self.world),
            GraphRef::Composite => Ok(&mut self.composite),
            GraphRef::Material(key) => self
                .materials
                .get_mut(key)
                .ok_or_else(|| Error::Missing(graph.clone())),
        }
    }

    #[must_use]
    pub fn tracks(&self) -> &Tracks<LookKey> {
        self.keys.tracks()
    }

    pub fn tracks_mut(&mut self) -> &mut Tracks<LookKey> {
        self.keys.tracks_mut()
    }

    #[must_use]
    pub fn last_frame(&self) -> u32 {
        self.keys.last_frame()
    }

    #[must_use]
    pub fn socket(&self, key: &SocketKey) -> Option<&SocketValue> {
        self.graph(&key.graph)?.socket(&key.socket)
    }

    #[must_use]
    pub fn socket_hint(&self, library: &Library, key: &SocketKey) -> Option<Hint> {
        port_hint(self.graph(&key.graph)?, library, &key.socket)
    }

    #[must_use]
    pub fn node_title(
        &self,
        library: &Library,
        graph: &GraphRef,
        scope: &Scope,
        node: NodeId,
    ) -> Option<String> {
        let kind = &self.graph(graph)?.scope(scope)?.node(node)?.kind;
        Some(kind.title(library))
    }

    pub fn apply(&mut self, library: &Library, edit: LookEdit) -> Result<LookEdit, Error> {
        Ok(match edit {
            LookEdit::Graph { graph, edit } => {
                let held = self.graph_mut(&graph)?;
                let undo =
                    Arc::make_mut(held)
                        .apply(library, edit)
                        .map_err(|source| Error::Edit {
                            graph: graph.clone(),
                            source: Box::new(source),
                        })?;
                let dropped = self.drop_stale(library, &graph);
                LookEdit::Batch(vec![
                    LookEdit::Keys(dropped),
                    LookEdit::Graph { graph, edit: undo },
                ])
            }
            LookEdit::Replace { graph, with } => {
                check_kind(&graph, &with)?;
                let previous = std::mem::replace(self.graph_mut(&graph)?, with);
                let dropped = self.drop_stale(library, &graph);
                LookEdit::Batch(vec![
                    LookEdit::Keys(dropped),
                    LookEdit::Replace {
                        graph,
                        with: previous,
                    },
                ])
            }
            LookEdit::InsertMaterial { key, graph } => {
                check_material_key(&key)?;
                let at = GraphRef::Material(key.clone());
                check_kind(&at, &graph)?;
                if self.materials.contains_key(&key) {
                    return Err(Error::Taken(at));
                }
                self.materials.insert(key.clone(), graph);
                LookEdit::RemoveMaterial { key }
            }
            LookEdit::RemoveMaterial { key } => {
                let at = GraphRef::Material(key.clone());
                let graph = self
                    .materials
                    .remove(&key)
                    .ok_or_else(|| Error::Missing(at.clone()))?;
                let slots = self.slots.remove_material(&key);
                let keys = self
                    .keys
                    .apply(self.keys.dropping(|target| target.graph() == Some(&at)));
                LookEdit::Batch(vec![
                    LookEdit::Keys(keys),
                    LookEdit::Slots(slots),
                    LookEdit::InsertMaterial { key, graph },
                ])
            }
            LookEdit::RenameMaterial { from, to } => {
                check_material_key(&to)?;
                if self.materials.contains_key(&to) {
                    return Err(Error::Taken(GraphRef::Material(to)));
                }
                let graph = self
                    .materials
                    .remove(&from)
                    .ok_or_else(|| Error::Missing(GraphRef::Material(from.clone())))?;
                self.materials.insert(to.clone(), graph);
                self.slots.rename_material(&from, &to);
                let renamed = self.keys.renaming_material(&from, &to);
                self.keys.apply(renamed);
                LookEdit::RenameMaterial { from: to, to: from }
            }
            LookEdit::Slots(edit) => LookEdit::Slots(self.slots.apply(edit)),
            LookEdit::Keys(edit) => LookEdit::Keys(self.keys.apply(edit)),
            LookEdit::Batch(edits) => {
                let mut undone = Vec::with_capacity(edits.len());
                for edit in edits {
                    match self.apply(library, edit) {
                        Ok(undo) => undone.push(undo),
                        Err(error) => {
                            for undo in undone.into_iter().rev() {
                                self.apply(library, undo)?;
                            }
                            return Err(error);
                        }
                    }
                }
                undone.reverse();
                LookEdit::Batch(undone)
            }
        })
    }

    pub fn key_socket(
        &mut self,
        library: &Library,
        key: SocketKey,
        frame: u32,
        value: SocketValue,
        curve: VmdCurve,
    ) -> Result<LookEdit, Error> {
        let held = self
            .graph(&key.graph)
            .ok_or_else(|| Error::Missing(key.graph.clone()))?;
        let hint = port_hint(held, library, &key.socket).ok_or_else(|| Error::Key {
            target: key.to_string(),
            reason: "the graph has no such input".into(),
        })?;
        let mut edits = Vec::new();
        if held.socket(&key.socket).is_none() {
            edits.push(LookEdit::Graph {
                graph: key.graph.clone(),
                edit: Edit::SetInput {
                    scope: key.socket.scope.clone(),
                    id: key.socket.node,
                    port: key.socket.port.clone(),
                    value: Some(value.clone()),
                },
            });
        }
        edits.push(LookEdit::Keys(self.keys.inserting(LookKey {
            target: Target::Socket(key),
            frame,
            value: LookValue::from_socket(&value, hint),
            curve,
        })));
        self.apply(library, LookEdit::Batch(edits))
    }

    fn drop_stale(&mut self, library: &Library, graph: &GraphRef) -> KeyEdit {
        let Some(held) = self.graph(graph).cloned() else {
            return KeyEdit::default();
        };
        let stale = self.keys.dropping(|target| match target {
            Target::Socket(key) => key.graph == *graph && !holds_input(&held, library, &key.socket),
            Target::Slot(_) => false,
        });
        self.keys.apply(stale)
    }

    pub(crate) fn keyed_sockets<'a>(
        &'a self,
        graph: &'a GraphRef,
    ) -> impl Iterator<Item = (&'a SocketRef, &'a LookKey)> + 'a {
        self.keys
            .tracks()
            .tracks()
            .filter_map(move |(target, keys)| {
                let Target::Socket(key) = target else {
                    return None;
                };
                if key.graph != *graph {
                    return None;
                }
                Some((&key.socket, keys.first()?))
            })
    }

    pub fn live_graph(&self, library: &Library, graph: &GraphRef) -> Result<Arc<Graph>, Error> {
        let held = self
            .graph(graph)
            .ok_or_else(|| Error::Missing(graph.clone()))?;
        let unset: Vec<(SocketRef, SocketValue)> = self
            .keyed_sockets(graph)
            .filter(|(socket, _)| held.socket(socket).is_none())
            .filter(|(socket, _)| holds_input(held, library, socket))
            .filter_map(|(socket, key)| Some((socket.clone(), key.value.to_socket()?)))
            .collect();
        if unset.is_empty() {
            return Ok(Arc::clone(held));
        }
        let mut live = Graph::clone(held);
        for (socket, value) in unset {
            let set = live.apply(
                library,
                Edit::SetInput {
                    scope: socket.scope.clone(),
                    id: socket.node,
                    port: socket.port.clone(),
                    value: Some(value),
                },
            );
            if let Err(source) = set {
                return Err(Error::Edit {
                    graph: graph.clone(),
                    source: Box::new(source),
                });
            }
        }
        Ok(Arc::new(live))
    }

    pub fn map_assets(
        &mut self,
        library: &Library,
        mut map: impl FnMut(&str) -> String,
    ) -> Result<(), Error> {
        let graphs: Vec<GraphRef> = [GraphRef::World, GraphRef::Composite]
            .into_iter()
            .chain(self.materials.keys().cloned().map(GraphRef::Material))
            .collect();
        for graph in &graphs {
            let held = self.graph_mut(graph)?;
            map_graph_assets(Arc::make_mut(held), library, &mut map).map_err(|source| {
                Error::Edit {
                    graph: graph.clone(),
                    source: Box::new(source),
                }
            })?;
        }
        let mut slots = Vec::new();
        for (figure, material, slot) in self.slots.iter() {
            let mut mapped = slot.clone();
            for value in mapped.values.values_mut() {
                if let LookValue::Texture(Some(texture)) = value {
                    texture.asset = Asset::new(&map(&texture.asset));
                }
            }
            if mapped != *slot {
                slots.push(SlotEdit::Replace {
                    figure,
                    material,
                    slot: Some(mapped),
                });
            }
        }
        self.slots.apply(SlotEdit::Batch(slots));
        let mut keys = KeyEdit::default();
        for key in self.keys.tracks().all_keys() {
            let file = match (&key.target, &key.value) {
                (Target::Socket(socket), LookValue::Asset(_)) => {
                    self.socket_hint(library, socket) != Some(Hint::Text)
                }
                (_, LookValue::Texture(Some(_))) => true,
                _ => false,
            };
            if !file {
                continue;
            }
            let mut mapped = key.clone();
            match &mut mapped.value {
                LookValue::Asset(path) => *path = Asset::new(&map(path)),
                LookValue::Texture(Some(texture)) => {
                    texture.asset = Asset::new(&map(&texture.asset));
                }
                _ => {}
            }
            keys.removed.push(key.clone());
            keys.added.push(mapped);
        }
        self.keys.apply(keys);
        Ok(())
    }
}

fn map_graph_assets(
    graph: &mut Graph,
    library: &Library,
    map: &mut impl FnMut(&str) -> String,
) -> Result<(), rikka_shader_graph::GraphError> {
    let mut edits = Vec::new();
    for (scope, body) in graph.scopes() {
        for (id, node) in body.nodes() {
            for (port, value) in &node.inputs {
                let SocketValue::Asset(path) = value else {
                    continue;
                };
                let socket = SocketRef::new(scope.clone(), *id, port.clone());
                if port_hint(graph, library, &socket) == Some(Hint::Text) {
                    continue;
                }
                let mapped = map(path);
                if mapped != *path {
                    edits.push(Edit::SetInput {
                        scope: scope.clone(),
                        id: *id,
                        port: port.clone(),
                        value: Some(SocketValue::Asset(mapped)),
                    });
                }
            }
        }
    }
    if edits.is_empty() {
        return Ok(());
    }
    graph.apply(library, Edit::Batch(edits)).map(|_| ())
}
