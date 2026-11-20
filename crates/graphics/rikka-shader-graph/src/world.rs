use std::collections::BTreeSet;

use crate::context::Site;
use crate::error::{Error, GraphError};
use crate::eval::{Compiler, Env, Program};
use crate::flat::{Flat, FlatNode, Path, Source};
use crate::graph::{Graph, GraphKind, NodeId, SocketRef};
use crate::library::{Behavior, Library};
use crate::node::NodeKind;
use crate::types::{Constant, PortType, ValueType};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FieldPlan {
    Number(usize),
    Asset(Option<SocketRef>),
    Record(Option<Box<RecordPlan>>),
    Records(Vec<RecordPlan>),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RecordPlan {
    pub id: String,
    pub path: Vec<NodeId>,
    pub fields: Vec<(String, FieldPlan)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Datum {
    Number(Constant),
    Asset(Option<String>),
    Record(Option<Record>),
    Records(Vec<Record>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub id: String,
    pub path: Vec<NodeId>,
    pub fields: Vec<(String, Datum)>,
}

impl Record {
    pub fn field(&self, name: &str) -> Option<&Datum> {
        self.fields
            .iter()
            .find(|(known, _)| known == name)
            .map(|(_, datum)| datum)
    }

    pub fn number(&self, name: &str) -> Option<Constant> {
        match self.field(name)? {
            Datum::Number(constant) => Some(*constant),
            _ => None,
        }
    }

    pub fn float(&self, name: &str) -> Option<f32> {
        self.number(name).map(|constant| constant.component(0))
    }

    pub fn asset(&self, name: &str) -> Option<&str> {
        match self.field(name)? {
            Datum::Asset(path) => path.as_deref(),
            _ => None,
        }
    }

    pub fn record(&self, name: &str) -> Option<&Record> {
        match self.field(name)? {
            Datum::Record(record) => record.as_ref(),
            _ => None,
        }
    }

    pub fn records(&self, name: &str) -> &[Record] {
        match self.field(name) {
            Some(Datum::Records(records)) => records,
            _ => &[],
        }
    }
}

fn record_node<'f, 'g>(flat: &'f Flat<'g>, path: &Path) -> Option<(&'f Path, &'f FlatNode<'g>)> {
    let (path, node) = flat.nodes.get_key_value(path)?;
    match node.kind {
        NodeKind::FunctionInput(_) | NodeKind::FunctionOutput(_) => match node.input("value") {
            Source::Link(from, _) => record_node(flat, from),
            _ => None,
        },
        NodeKind::Extern(_) => {
            matches!(flat.spec(node)?.behavior, Behavior::Record).then_some((path, node))
        }
        _ => None,
    }
}

pub(crate) fn fields(
    compiler: &mut Compiler<'_, '_>,
    flat: &Flat<'_>,
    node: &FlatNode<'_>,
) -> Vec<(String, FieldPlan)> {
    let types = flat.types(node);
    types
        .ports
        .inputs
        .iter()
        .filter_map(|port| {
            let ty = types.inputs[&port.name];
            let plan = match ty {
                ValueType::Image | ValueType::Shader => return None,
                ValueType::Texture => FieldPlan::Asset(match node.input(&port.name) {
                    Source::Socket(at, _) => Some(at.clone()),
                    _ => None,
                }),
                ValueType::Record(_) if port.many => FieldPlan::Records(
                    node.input(&port.name)
                        .links()
                        .iter()
                        .filter_map(|(from, _)| record_node(flat, from))
                        .map(|(path, record)| plan(compiler, flat, path, record))
                        .collect(),
                ),
                ValueType::Record(_) => FieldPlan::Record(match node.input(&port.name) {
                    Source::Link(from, _) => record_node(flat, from)
                        .map(|(path, record)| Box::new(plan(compiler, flat, path, record))),
                    _ => None,
                }),
                _ => FieldPlan::Number(compiler.input(node, &port.name)),
            };
            Some((port.name.clone(), plan))
        })
        .collect()
}

fn plan(
    compiler: &mut Compiler<'_, '_>,
    flat: &Flat<'_>,
    path: &Path,
    node: &FlatNode<'_>,
) -> RecordPlan {
    RecordPlan {
        id: match node.kind {
            NodeKind::Extern(id) => id.clone(),
            _ => String::new(),
        },
        path: path.clone(),
        fields: fields(compiler, flat, node),
    }
}

pub(crate) fn resolve(plan: &RecordPlan, env: &Env<'_>, values: &[Constant]) -> Record {
    Record {
        id: plan.id.clone(),
        path: plan.path.clone(),
        fields: plan
            .fields
            .iter()
            .map(|(name, field)| {
                let datum = match field {
                    FieldPlan::Number(step) => Datum::Number(values[*step]),
                    FieldPlan::Asset(at) => Datum::Asset(at.as_ref().and_then(|at| {
                        (env.sockets)(at).and_then(|held| held.asset().map(str::to_owned))
                    })),
                    FieldPlan::Record(record) => {
                        Datum::Record(record.as_ref().map(|record| resolve(record, env, values)))
                    }
                    FieldPlan::Records(records) => Datum::Records(
                        records
                            .iter()
                            .map(|record| resolve(record, env, values))
                            .collect(),
                    ),
                };
                (name.clone(), datum)
            })
            .collect(),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorldPlan {
    program: Program,
    root: RecordPlan,
}

impl RecordPlan {
    fn find(&self, path: &[NodeId]) -> Option<&RecordPlan> {
        if self.path == path {
            return Some(self);
        }
        self.fields.iter().find_map(|(_, field)| match field {
            FieldPlan::Record(Some(record)) => record.find(path),
            FieldPlan::Records(records) => records.iter().find_map(|record| record.find(path)),
            FieldPlan::Number(_) | FieldPlan::Asset(_) | FieldPlan::Record(None) => None,
        })
    }
}

impl WorldPlan {
    pub fn evaluate(&self, env: &Env<'_>) -> Record {
        let mut values = Vec::new();
        self.program.run(env, &mut values);
        resolve(&self.root, env, &values)
    }

    pub fn program(&self) -> &Program {
        &self.program
    }

    pub fn step(&self, path: &[NodeId], field: &str) -> Option<usize> {
        let record = self.root.find(path)?;
        record.fields.iter().find_map(|(name, plan)| match plan {
            FieldPlan::Number(step) if name == field => Some(*step),
            _ => None,
        })
    }
}

impl Graph {
    pub fn world(&self, library: &Library) -> Result<WorldPlan, Error> {
        let report = self.validate(library);
        if !report.is_valid() || self.kind() != GraphKind::World {
            return Err(Error(report.errors));
        }
        let flat = Flat::new(self, library, &report);
        let (path, output) = flat.output().expect("a checked graph has an output");
        let mut compiler = Compiler::new(&flat);
        let root = plan(&mut compiler, &flat, path, output);
        Ok(WorldPlan {
            program: compiler.finish(),
            root,
        })
    }
}

pub(crate) fn uniform_errors(flat: &Flat<'_>, node: &FlatNode<'_>) -> Vec<GraphError> {
    let types = flat.types(node);
    let roots = types
        .ports
        .inputs
        .iter()
        .filter(|port| match port.ty {
            PortType::Dynamic => true,
            PortType::Value(ty) => ty.is_number(),
        })
        .filter_map(|port| match node.input(&port.name) {
            Source::Link(from, _) => Some(from.clone()),
            _ => None,
        });
    flat.site_errors(&flat.cone(roots), Site::Uniform)
}

pub(crate) fn diagnose(_library: &Library, flat: &Flat<'_>) -> Vec<GraphError> {
    let mut errors = Vec::new();
    let Some((path, _)) = flat.output() else {
        return errors;
    };
    let mut seen = BTreeSet::new();
    let mut pending = vec![path.clone()];
    while let Some(path) = pending.pop() {
        if !seen.insert(path.clone()) {
            continue;
        }
        let node = &flat.nodes[&path];
        errors.extend(uniform_errors(flat, node));
        for (port, from, _) in node.sources() {
            if matches!(
                flat.types(node).inputs.get(port),
                Some(ValueType::Record(_))
            ) && let Some((record, _)) = record_node(flat, from)
            {
                pending.push(record.clone());
            }
        }
    }
    errors
}
