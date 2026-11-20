use std::hash::{Hash, Hasher};
use std::sync::Arc;

use rustc_hash::{FxHashMap, FxHasher};

use crate::document::Node;
use crate::read::props::Props;

pub(crate) struct Object<'a> {
    pub id: i64,
    pub kind: &'a str,
    pub name: String,
    pub sub_kind: String,
    pub node: &'a Node,
    pub props: Props,
}

impl Object<'_> {
    pub(crate) fn is(&self, kind: &str, sub_kind: &str) -> bool {
        self.kind == kind && self.sub_kind == sub_kind
    }
}

struct Connection {
    src: i64,
    dst: i64,
    property: Option<String>,
}

pub(crate) struct Graph<'a> {
    objects: Vec<Object<'a>>,
    by_id: FxHashMap<i64, usize>,
    connections: Vec<Connection>,
    by_src: FxHashMap<i64, Vec<usize>>,
    by_dst: FxHashMap<i64, Vec<usize>>,
}

impl<'a> Graph<'a> {
    pub(crate) fn read(nodes: &'a [Node], version: u32, ascii: bool) -> Self {
        let templates = templates(nodes);
        let legacy = version < 7000;
        let mut graph = Self {
            objects: Vec::new(),
            by_id: FxHashMap::default(),
            connections: Vec::new(),
            by_src: FxHashMap::default(),
            by_dst: FxHashMap::default(),
        };

        for block in nodes.iter().filter(|node| node.name == "Objects") {
            for node in &block.children {
                let (id, packed) = match legacy {
                    true => match node.text(0) {
                        Some(packed) => (synthetic(packed), packed),
                        None => continue,
                    },
                    false => match (node.integer(0), node.text(1)) {
                        (Some(id), Some(packed)) => (id, packed),
                        _ => continue,
                    },
                };
                let (name, _) = split(packed, ascii);
                let index = node.values.len().saturating_sub(1);
                graph.push(Object {
                    id,
                    kind: &node.name,
                    name,
                    sub_kind: node.text(index).unwrap_or_default().to_owned(),
                    node,
                    props: Props::read(node)
                        .with_template(templates.get(node.name.as_str()).cloned()),
                });
            }
        }

        for block in nodes.iter().filter(|node| node.name == "Connections") {
            for node in &block.children {
                let Some(kind) = node.text(0) else {
                    continue;
                };
                let (src, dst) = match legacy {
                    true => match (node.text(1), node.text(2)) {
                        (Some(src), Some(dst)) => (synthetic(src), synthetic(dst)),
                        _ => continue,
                    },
                    false => match (node.integer(1), node.integer(2)) {
                        (Some(src), Some(dst)) => (src, dst),
                        _ => continue,
                    },
                };
                let property = match kind {
                    "OP" | "PP" => node.text(3).map(str::to_owned),
                    _ => None,
                };
                graph.connect(Connection { src, dst, property });
            }
        }
        graph
    }

    fn push(&mut self, object: Object<'a>) {
        self.by_id.entry(object.id).or_insert(self.objects.len());
        self.objects.push(object);
    }

    fn connect(&mut self, connection: Connection) {
        let index = self.connections.len();
        self.by_src.entry(connection.src).or_default().push(index);
        self.by_dst.entry(connection.dst).or_default().push(index);
        self.connections.push(connection);
    }

    pub(crate) fn object(&self, id: i64) -> Option<&Object<'a>> {
        Some(&self.objects[*self.by_id.get(&id)?])
    }

    pub(crate) fn objects(&self) -> impl Iterator<Item = &Object<'a>> {
        self.objects.iter()
    }

    pub(crate) fn sources(&self, dst: i64) -> impl Iterator<Item = (&Object<'a>, Option<&str>)> {
        self.linked(&self.by_dst, dst, |connection| connection.src)
    }

    pub(crate) fn destinations(
        &self,
        src: i64,
    ) -> impl Iterator<Item = (&Object<'a>, Option<&str>)> {
        self.linked(&self.by_src, src, |connection| connection.dst)
    }

    fn linked(
        &self,
        index: &FxHashMap<i64, Vec<usize>>,
        id: i64,
        other: fn(&Connection) -> i64,
    ) -> impl Iterator<Item = (&Object<'a>, Option<&str>)> {
        index
            .get(&id)
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .filter_map(move |&connection_index| {
                let connection = &self.connections[connection_index];
                let object = self.object(other(connection))?;
                Some((object, connection.property.as_deref()))
            })
    }
}

fn split(packed: &str, ascii: bool) -> (String, String) {
    match ascii {
        true => match packed.split_once("::") {
            Some((class, name)) => (name.to_owned(), class.to_owned()),
            None => (packed.to_owned(), String::new()),
        },
        false => match packed.split_once('\u{0}') {
            Some((name, class)) => (
                name.to_owned(),
                class.trim_start_matches('\u{1}').to_owned(),
            ),
            None => (packed.to_owned(), String::new()),
        },
    }
}

pub(crate) fn synthetic(packed: &str) -> i64 {
    let mut hasher = FxHasher::default();
    packed.hash(&mut hasher);
    (hasher.finish() >> 1) as i64 | 1
}

fn templates(nodes: &[Node]) -> FxHashMap<String, Arc<Props>> {
    let mut templates = FxHashMap::default();
    for block in nodes.iter().filter(|node| node.name == "Definitions") {
        for kind in block.children_named("ObjectType") {
            let Some(name) = kind.text(0) else {
                continue;
            };
            let Some(template) = kind.child("PropertyTemplate") else {
                continue;
            };
            let props = Props::read(template);
            if !props.is_empty() {
                templates.insert(name.to_owned(), Arc::new(props));
            }
        }
    }
    templates
}
