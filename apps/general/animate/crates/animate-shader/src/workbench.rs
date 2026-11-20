use std::collections::BTreeMap;
use std::sync::Arc;

use animate_look::{GraphRef, Look, LookEdit, SocketKey, check_material_key};
use rikka_shader_graph::{
    Change, CompositePlan, Graph, GraphError, Library, SocketValue, WorldPlan,
};
use rikka_sync::Mutex;

use crate::diagnostic::{self, Diagnostic};
use crate::error::{Error, Result};
use animate_look::fresh_name;

pub type Planned<T> = Result<Arc<T>, Arc<[Diagnostic]>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Merge(pub u64);
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Synced {
    Changed(GraphRef),
    Renamed { from: String, to: String },
    Removed(String),
}

#[derive(Debug)]
struct Seen {
    graph: Arc<Graph>,
    revision: u64,
}

#[derive(Debug)]
struct Cached<T> {
    slot: Mutex<Option<(u64, Planned<T>)>>,
}

impl<T> Default for Cached<T> {
    fn default() -> Self {
        Self {
            slot: Mutex::new(None),
        }
    }
}

impl<T> Cached<T> {
    fn get(
        &self,
        revision: u64,
        plan: impl FnOnce() -> Result<T, rikka_shader_graph::Error>,
    ) -> Planned<T> {
        let mut slot = self.slot.lock();
        if let Some((known, planned)) = slot.as_ref()
            && *known == revision
        {
            return planned.clone();
        }
        let planned = plan()
            .map(Arc::new)
            .map_err(|error| Arc::from(diagnostic::from_graph(error.errors())));
        *slot = Some((revision, planned.clone()));
        planned
    }
}

#[derive(Debug)]
pub struct Workbench {
    library: Arc<Library>,
    seen: BTreeMap<GraphRef, Seen>,
    counter: u64,
    world: Cached<WorldPlan>,
    composite: Cached<CompositePlan>,
}

impl Workbench {
    pub fn new(library: Arc<Library>) -> Self {
        Self {
            library,
            seen: BTreeMap::new(),
            counter: 0,
            world: Cached::default(),
            composite: Cached::default(),
        }
    }

    pub fn library(&self) -> &Arc<Library> {
        &self.library
    }

    pub fn sync(&mut self, look: &Look) -> Vec<Synced> {
        let gone: Vec<(GraphRef, Seen)> = {
            let keys: Vec<GraphRef> = self
                .seen
                .keys()
                .filter(|at| look.graph(at).is_none())
                .cloned()
                .collect();
            keys.into_iter()
                .filter_map(|at| self.seen.remove_entry(&at))
                .collect()
        };
        let mut synced = Vec::new();
        for (at, graph) in look.graphs() {
            let current = self
                .seen
                .get(&at)
                .is_some_and(|seen| Arc::ptr_eq(&seen.graph, graph));
            if current {
                continue;
            }
            let moved = gone
                .iter()
                .find(|(_, seen)| Arc::ptr_eq(&seen.graph, graph))
                .and_then(|(from, _)| Some((from.material()?, at.material()?)));
            synced.push(match moved {
                Some((from, to)) => Synced::Renamed {
                    from: from.to_owned(),
                    to: to.to_owned(),
                },
                None => Synced::Changed(at.clone()),
            });
            self.counter += 1;
            self.seen.insert(
                at,
                Seen {
                    graph: Arc::clone(graph),
                    revision: self.counter,
                },
            );
        }
        for (at, _) in gone {
            let renamed = synced.iter().any(
                |synced| matches!(synced, Synced::Renamed { from, .. } if at.material() == Some(from)),
            );
            if let (false, Some(key)) = (renamed, at.material()) {
                synced.push(Synced::Removed(key.to_owned()));
            }
        }
        synced
    }

    pub fn revision(&self) -> u64 {
        self.counter
    }

    pub fn graph_revision(&self, at: &GraphRef) -> Option<u64> {
        self.seen.get(at).map(|seen| seen.revision)
    }

    pub fn world_plan(&self, look: &Look) -> Planned<WorldPlan> {
        let revision = self.graph_revision(&GraphRef::World).unwrap_or(0);
        self.world.get(revision, || look.world.world(&self.library))
    }

    pub fn composite_plan(&self, look: &Look) -> Planned<CompositePlan> {
        let revision = self.graph_revision(&GraphRef::Composite).unwrap_or(0);
        self.composite
            .get(revision, || look.composite.composite(&self.library))
    }

    pub fn change<T>(
        &self,
        look: &Look,
        at: &GraphRef,
        edit: impl FnOnce(&mut Graph, &Library) -> Result<Change<T>, GraphError>,
    ) -> Result<(T, LookEdit)> {
        let held = look
            .graph(at)
            .ok_or_else(|| animate_look::Error::Missing(at.clone()))?;
        let mut graph = Graph::clone(held);
        let Change { value, .. } = edit(&mut graph, &self.library)?;
        Ok((
            value,
            LookEdit::Replace {
                graph: at.clone(),
                with: Arc::new(graph),
            },
        ))
    }

    pub fn set_socket(&self, key: &SocketKey, value: Option<SocketValue>) -> LookEdit {
        LookEdit::Graph {
            graph: key.graph.clone(),
            edit: key.edit(value),
        }
    }

    pub fn fresh_material(&self, look: &Look, base: &str) -> String {
        fresh_name(base, |key| look.materials.contains_key(key))
    }

    pub fn add_material(
        &self,
        look: &Look,
        base: &str,
        graph: Graph,
    ) -> Result<(String, LookEdit)> {
        let key = self.fresh_material(look, base);
        check_material_key(&key)?;
        self.check(&GraphRef::Material(key.clone()), &graph)?;
        Ok((
            key.clone(),
            LookEdit::InsertMaterial {
                key,
                graph: Arc::new(graph),
            },
        ))
    }

    pub fn duplicate_material(&self, look: &Look, key: &str) -> Result<(String, LookEdit)> {
        let graph = look
            .materials
            .get(key)
            .ok_or_else(|| animate_look::Error::Missing(GraphRef::Material(key.to_owned())))?;
        self.add_material(look, key, Graph::clone(graph))
    }

    fn check(&self, at: &GraphRef, graph: &Graph) -> Result<()> {
        let errors: Vec<GraphError> = graph
            .validate(&self.library)
            .errors()
            .iter()
            .filter(|error| !error.problem.is_incomplete())
            .cloned()
            .collect();
        if graph.kind() != at.kind() {
            return Err(animate_look::Error::Kind {
                graph: at.clone(),
                expected: at.kind().name(),
                found: graph.kind().name(),
            }
            .into());
        }
        match errors.is_empty() {
            true => Ok(()),
            false => Err(Error::Built(rikka_shader_graph::Error(errors))),
        }
    }
}
