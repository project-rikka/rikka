use std::collections::HashSet;

use serde_json::{Value, json};

use crate::error::{Error, Limit, Result};

pub(crate) const MAX_UNIT_CHARS: usize = 20_000;
pub(crate) const MAX_DEPTH: usize = 32;
pub(crate) const MAX_ID: u32 = 1 << 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unit {
    pub(crate) pre: bool,
    pub(crate) nodes: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Text(String),
    Keep { id: u32, text: String },
    Element { id: u32, nodes: Vec<Node> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    pub nodes: Vec<OutputNode>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutputNode {
    Text(String),
    Keep(u32),
    Element { id: u32, nodes: Vec<OutputNode> },
}

impl Unit {
    pub fn new(nodes: Vec<Node>) -> Result<Unit> {
        Validator::default().nodes(&nodes, 0)?;
        Ok(Unit { pre: false, nodes })
    }

    #[must_use]
    pub fn preformatted(mut self) -> Unit {
        self.pre = true;
        self
    }

    pub(crate) fn parse(value: &Value) -> Result<Unit> {
        let object = value.as_object().ok_or(Error::Malformed("unit"))?;
        let pre = match object.get("pre") {
            None | Some(Value::Null) => false,
            Some(pre) => pre.as_bool().ok_or(Error::Malformed("pre"))?,
        };
        let nodes = object
            .get("nodes")
            .and_then(Value::as_array)
            .ok_or(Error::Malformed("nodes"))?;
        let unit = Unit::new(parse_nodes(nodes, 0)?)?;
        Ok(match pre {
            true => unit.preformatted(),
            false => unit,
        })
    }

    pub(crate) fn has_text(&self) -> bool {
        fn any(nodes: &[Node]) -> bool {
            nodes.iter().any(|node| match node {
                Node::Text(text) => !text.trim().is_empty(),
                Node::Keep { .. } => false,
                Node::Element { nodes, .. } => any(nodes),
            })
        }
        any(&self.nodes)
    }

    pub(crate) fn untouched(&self) -> Output {
        fn copy(nodes: &[Node]) -> Vec<OutputNode> {
            nodes
                .iter()
                .map(|node| match node {
                    Node::Text(text) => OutputNode::Text(text.clone()),
                    Node::Keep { id, .. } => OutputNode::Keep(*id),
                    Node::Element { id, nodes } => OutputNode::Element {
                        id: *id,
                        nodes: copy(nodes),
                    },
                })
                .collect()
        }
        Output {
            nodes: copy(&self.nodes),
        }
    }
}

impl Output {
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({ "nodes": outs_json(&self.nodes) })
    }
}

fn outs_json(nodes: &[OutputNode]) -> Vec<Value> {
    nodes
        .iter()
        .map(|node| match node {
            OutputNode::Text(text) => Value::String(text.clone()),
            OutputNode::Keep(id) => json!({ "i": id }),
            OutputNode::Element { id, nodes } => json!({ "i": id, "nodes": outs_json(nodes) }),
        })
        .collect()
}

fn parse_nodes(values: &[Value], depth: usize) -> Result<Vec<Node>> {
    if depth > MAX_DEPTH {
        return Err(Error::TooLarge(Limit::Depth));
    }
    values
        .iter()
        .map(|value| parse_node(value, depth))
        .collect()
}

fn parse_node(value: &Value, depth: usize) -> Result<Node> {
    if let Some(text) = value.as_str() {
        return Ok(Node::Text(text.to_owned()));
    }
    let object = value.as_object().ok_or(Error::Malformed("node"))?;
    let id = object
        .get("i")
        .and_then(Value::as_u64)
        .ok_or(Error::Malformed("i"))?;
    let id = u32::try_from(id)
        .ok()
        .filter(|id| *id <= MAX_ID)
        .ok_or(Error::TooLarge(Limit::Id))?;
    match (object.get("keep"), object.get("nodes")) {
        (Some(keep), None) => Ok(Node::Keep {
            id,
            text: keep.as_str().ok_or(Error::Malformed("keep"))?.to_owned(),
        }),
        (None, Some(nodes)) => Ok(Node::Element {
            id,
            nodes: parse_nodes(
                nodes.as_array().ok_or(Error::Malformed("nodes"))?,
                depth + 1,
            )?,
        }),
        _ => Err(Error::Malformed("keep or nodes")),
    }
}

#[derive(Default)]
struct Validator {
    ids: HashSet<u32>,
    chars: usize,
}

impl Validator {
    fn nodes(&mut self, nodes: &[Node], depth: usize) -> Result<()> {
        if depth > MAX_DEPTH {
            return Err(Error::TooLarge(Limit::Depth));
        }
        for node in nodes {
            match node {
                Node::Text(text) => self.text(text)?,
                Node::Keep { id, text } => {
                    self.id(*id)?;
                    self.text(text)?;
                }
                Node::Element { id, nodes } => {
                    self.id(*id)?;
                    self.nodes(nodes, depth + 1)?;
                }
            }
        }
        Ok(())
    }

    fn id(&mut self, id: u32) -> Result<()> {
        if id > MAX_ID {
            return Err(Error::TooLarge(Limit::Id));
        }
        if !self.ids.insert(id) {
            return Err(Error::DuplicateId(id));
        }
        Ok(())
    }

    fn text(&mut self, text: &str) -> Result<()> {
        self.chars += text.chars().count();
        if self.chars > MAX_UNIT_CHARS {
            return Err(Error::TooLarge(Limit::UnitText));
        }
        Ok(())
    }
}
