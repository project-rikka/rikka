use std::collections::BTreeMap;
use std::sync::{Arc, Weak};

use rikka_shader_compile::Material;
use rikka_shader_graph::{Constant, Edit, Error, Graph, Library, SocketValue};
use rikka_sync::Mutex;

use crate::compile::{Job, Outcome};
use crate::diagnostic::{self, Diagnostic};
use crate::names::compile_name;

pub type Listener = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Compiling,
    Ready,
    Failed(Arc<[Diagnostic]>),
}

#[derive(Clone, Debug)]
enum Wanted {
    Source(Arc<str>),
    Invalid(Arc<[Diagnostic]>),
}

#[derive(Clone, Debug)]
struct Attempt {
    signature: Arc<str>,
    failure: Option<Arc<[Diagnostic]>>,
}

#[derive(Clone, Debug, Default)]
struct Entry {
    seen: Option<u64>,
    wanted: Option<Wanted>,
    compiling: Option<Arc<str>>,
    queued: Option<Job>,
    attempted: Option<Attempt>,
    material: Option<Arc<Material>>,
}

pub type SharedShaders = Arc<Mutex<Shaders>>;

#[derive(Default)]
pub struct Shaders {
    entries: BTreeMap<String, Entry>,
    listeners: Vec<Weak<dyn Fn() + Send + Sync>>,
}

impl std::fmt::Debug for Shaders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shaders")
            .field("entries", &self.entries)
            .field("listeners", &self.listeners.len())
            .finish()
    }
}

impl Shaders {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn watch(&mut self, listener: &Listener) {
        self.listeners.push(Arc::downgrade(listener));
    }

    pub fn listeners(&mut self) -> Vec<Listener> {
        self.listeners
            .retain(|listener| listener.strong_count() > 0);
        self.listeners.iter().filter_map(Weak::upgrade).collect()
    }

    pub fn request(
        &mut self,
        key: &str,
        revision: u64,
        graph: &Graph,
        library: &Arc<Library>,
    ) -> Option<Job> {
        let entry = self.entries.entry(key.to_owned()).or_default();
        if entry.seen == Some(revision) {
            return None;
        }
        entry.seen = Some(revision);
        let name = entry
            .material
            .as_ref()
            .map_or_else(|| compile_name(key), |live| live.source().name.clone());
        let source: Arc<str> = match signature(graph, library, &name) {
            Ok(source) => source.into(),
            Err(error) => {
                entry.wanted = Some(Wanted::Invalid(
                    diagnostic::from_graph(error.errors()).into(),
                ));
                entry.queued = None;
                return None;
            }
        };
        entry.wanted = Some(Wanted::Source(Arc::clone(&source)));
        let covered = match (&entry.compiling, &entry.attempted) {
            (Some(compiling), _) => **compiling == *source,
            (None, Some(attempt)) => *attempt.signature == *source,
            (None, None) => false,
        };
        if covered {
            entry.queued = None;
            return None;
        }
        let job = Job {
            key: key.to_owned(),
            revision,
            name,
            signature: source,
            graph: graph.clone(),
            library: Arc::clone(library),
            material: entry.material.clone(),
        };
        if entry.compiling.is_some() {
            entry.queued = Some(job);
            return None;
        }
        entry.compiling = Some(Arc::clone(&job.signature));
        Some(job)
    }

    pub fn finish(&mut self, outcome: Outcome) -> Option<Job> {
        let entry = self.entries.values_mut().find(|entry| {
            entry
                .compiling
                .as_ref()
                .is_some_and(|compiling| Arc::ptr_eq(compiling, &outcome.signature))
        })?;
        entry.compiling = None;
        let failure = match outcome.result {
            Ok(material) => {
                entry.material = Some(Arc::new(material));
                None
            }
            Err(diagnostics) => Some(Arc::from(diagnostics)),
        };
        entry.attempted = Some(Attempt {
            signature: outcome.signature,
            failure,
        });
        let mut job = entry.queued.take()?;
        job.material = entry.material.clone();
        entry.compiling = Some(Arc::clone(&job.signature));
        Some(job)
    }

    pub fn status(&self, key: &str) -> Option<Status> {
        let entry = self.entries.get(key)?;
        if let Some(Wanted::Invalid(diagnostics)) = &entry.wanted {
            return Some(Status::Failed(Arc::clone(diagnostics)));
        }
        if entry.compiling.is_some() || entry.queued.is_some() {
            return Some(Status::Compiling);
        }
        if let (Some(Wanted::Source(wanted)), Some(attempt)) = (&entry.wanted, &entry.attempted)
            && let Some(failure) = &attempt.failure
            && *wanted == attempt.signature
        {
            return Some(Status::Failed(Arc::clone(failure)));
        }
        Some(match entry.material {
            Some(_) => Status::Ready,
            None => Status::Compiling,
        })
    }

    pub fn diagnostics(&self, key: &str) -> Arc<[Diagnostic]> {
        match self.status(key) {
            Some(Status::Failed(diagnostics)) => diagnostics,
            _ => Arc::new([]),
        }
    }

    pub fn material(&self, key: &str) -> Option<Arc<Material>> {
        self.entries.get(key)?.material.clone()
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    pub fn forget(&mut self, key: &str) {
        self.entries.remove(key);
    }

    pub fn rename(&mut self, from: &str, to: &str) {
        if let Some(mut entry) = self.entries.remove(from) {
            if let Some(job) = entry.queued.as_mut() {
                job.key = to.to_owned();
            }
            self.entries.insert(to.to_owned(), entry);
        }
    }
}

fn signature(graph: &Graph, library: &Library, name: &str) -> Result<String, Error> {
    let resets: Vec<Edit> = graph
        .scopes()
        .flat_map(|(scope, body)| {
            body.nodes().iter().flat_map(move |(id, node)| {
                let scope = scope.clone();
                node.inputs.iter().filter_map(move |(port, value)| {
                    let SocketValue::Number(constant) = value else {
                        return None;
                    };
                    let zero = Constant::from_slice(&[0.0; 4][..constant.width()])?;
                    Some(Edit::SetInput {
                        scope: scope.clone(),
                        id: *id,
                        port: port.clone(),
                        value: Some(SocketValue::Number(zero)),
                    })
                })
            })
        })
        .collect();
    let mut shape = graph.clone();
    shape
        .apply(library, Edit::Batch(resets))
        .map_err(|error| Error(vec![error]))?;
    Ok(shape.material(library, name)?.source)
}
