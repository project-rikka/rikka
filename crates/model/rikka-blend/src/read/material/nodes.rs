use rikka_model_core::glam::{Vec2, Vec4};
use rustc_hash::FxHashMap;

use crate::file::Record;

const NODE_DO_OUTPUT: i64 = 1 << 6;
const NODE_LINK_MUTED: i64 = 1 << 4;
const NODE_MUTED: i64 = 1 << 9;
const OUTPUT_CYCLES: i64 = 2;
const SOCK_FLOAT: i64 = 0;
const SOCK_VECTOR: i64 = 1;
const SOCK_RGBA: i64 = 2;
const SOCK_BOOLEAN: i64 = 4;
const SOCK_INT: i64 = 6;
const NODE_GROUP: i64 = 2;
const GROUP_DEPTH: usize = 8;

#[derive(Clone)]
pub(crate) struct Socket<'a> {
    pub(crate) record: Record<'a>,
    pub(crate) name: String,
    pub(crate) identifier: String,
}

impl<'a> Socket<'a> {
    fn value(&self) -> Option<Record<'a>> {
        let default_value = self.record.deref("default_value")?;
        if default_value.kind().starts_with("bNodeSocketValue") {
            return Some(default_value);
        }
        let layout = match self.record.int("type")? {
            SOCK_FLOAT => "bNodeSocketValueFloat",
            SOCK_VECTOR => "bNodeSocketValueVector",
            SOCK_RGBA => "bNodeSocketValueRGBA",
            SOCK_BOOLEAN => "bNodeSocketValueBoolean",
            SOCK_INT => "bNodeSocketValueInt",
            _ => return None,
        };
        self.record.deref_as("default_value", layout)
    }

    pub(crate) fn color(&self) -> Option<Vec4> {
        let value = self.value()?;
        match value.kind() {
            "bNodeSocketValueRGBA" => value.floats::<4>("value").map(Vec4::from_array),
            "bNodeSocketValueFloat" => value.float("value").map(Vec4::splat),
            _ => None,
        }
    }

    pub(crate) fn float(&self) -> Option<f32> {
        let value = self.value()?;
        match value.kind() {
            "bNodeSocketValueFloat" | "bNodeSocketValueInt" => value.float("value"),
            "bNodeSocketValueBoolean" => value.int("value").map(|value| value as f32),
            "bNodeSocketValueRGBA" => value
                .floats::<4>("value")
                .map(|color| 0.2126 * color[0] + 0.7152 * color[1] + 0.0722 * color[2]),
            _ => None,
        }
    }

    pub(crate) fn vector(&self) -> Option<Vec4> {
        let value = self.value()?;
        value
            .floats::<4>("value")
            .map(Vec4::from_array)
            .or_else(|| {
                value
                    .floats::<3>("value")
                    .map(|vector| Vec4::from((vector[0], vector[1], vector[2], 0.0)))
            })
    }

    pub(crate) fn is_color(&self) -> bool {
        self.value()
            .is_some_and(|value| value.kind() == "bNodeSocketValueRGBA")
    }
}

pub(crate) struct Node<'a> {
    pub(crate) record: Record<'a>,
    pub(crate) idname: String,
    pub(crate) legacy: i64,
    pub(crate) inputs: Vec<Socket<'a>>,
    pub(crate) outputs: Vec<Socket<'a>>,
    pub(crate) muted: bool,
}

impl<'a> Node<'a> {
    pub(crate) fn input(&self, names: &[&str]) -> Option<usize> {
        names.iter().find_map(|name| {
            self.inputs
                .iter()
                .position(|socket| socket.identifier == *name)
                .or_else(|| self.inputs.iter().position(|socket| socket.name == *name))
        })
    }

    pub(crate) fn is(&self, idname: &str, legacy: i64) -> bool {
        self.idname == idname || (self.idname.is_empty() && self.legacy == legacy)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Link {
    pub(crate) node: usize,
    pub(crate) socket: usize,
}

pub(crate) struct Tree<'a> {
    pub(crate) nodes: Vec<Node<'a>>,
    links: FxHashMap<(usize, usize), Link>,
}

impl<'a> Tree<'a> {
    pub(crate) fn read(tree: Record<'a>) -> Self {
        Self::read_nested(tree, 0)
    }

    fn read_nested(tree: Record<'a>, depth: usize) -> Self {
        let records = tree.list("nodes");
        let mut sockets: FxHashMap<u64, (usize, usize, bool)> = FxHashMap::default();
        let mut node_index: FxHashMap<u64, usize> = FxHashMap::default();
        let mut nodes = Vec::with_capacity(records.len());
        for (index, record) in records.iter().enumerate() {
            if let Some(address) = record.address() {
                node_index.insert(address, index);
            }
            let mut read_sockets = |list: &str, output: bool| -> Vec<Socket<'a>> {
                record
                    .list(list)
                    .into_iter()
                    .enumerate()
                    .map(|(slot, socket)| {
                        if let Some(address) = socket.address() {
                            sockets.insert(address, (index, slot, output));
                        }
                        Socket {
                            name: socket.text("name").unwrap_or_default(),
                            identifier: socket.text("identifier").unwrap_or_default(),
                            record: socket,
                        }
                    })
                    .collect()
            };
            let inputs = read_sockets("inputs", false);
            let outputs = read_sockets("outputs", true);
            nodes.push(Node {
                idname: record.text("idname").unwrap_or_default(),
                legacy: record
                    .int("type")
                    .or_else(|| record.int("type_legacy"))
                    .unwrap_or(0),
                muted: record.int("flag").unwrap_or(0) & NODE_MUTED != 0,
                record: *record,
                inputs,
                outputs,
            });
        }
        let mut links = FxHashMap::default();
        for link in tree.list("links") {
            if link.int("flag").unwrap_or(0) & NODE_LINK_MUTED != 0 {
                continue;
            }
            let (Some(from), Some(to)) = (link.pointer("fromsock"), link.pointer("tosock")) else {
                continue;
            };
            let (Some(&(from_node, from_socket, true)), Some(&(to_node, to_socket, false))) =
                (sockets.get(&from), sockets.get(&to))
            else {
                continue;
            };
            links.insert(
                (to_node, to_socket),
                Link {
                    node: from_node,
                    socket: from_socket,
                },
            );
        }
        let mut read = Self { nodes, links };
        if depth < GROUP_DEPTH {
            let groups: Vec<usize> = (0..read.nodes.len())
                .filter(|&index| read.nodes[index].is("ShaderNodeGroup", NODE_GROUP))
                .collect();
            for group in groups {
                if let Some(inner) = read.nodes[group].record.id("id") {
                    read.inline(group, Self::read_nested(inner, depth + 1));
                }
            }
        }
        read
    }

    fn inline(&mut self, group: usize, inner: Self) {
        let offset = self.nodes.len();
        let input_node = inner
            .nodes
            .iter()
            .position(|node| node.idname == "NodeGroupInput")
            .map(|index| index + offset);
        let output_node = inner
            .nodes
            .iter()
            .position(|node| {
                node.idname == "NodeGroupOutput"
                    && node.record.int("flag").unwrap_or(0) & NODE_DO_OUTPUT != 0
            })
            .or_else(|| {
                inner
                    .nodes
                    .iter()
                    .position(|node| node.idname == "NodeGroupOutput")
            })
            .map(|index| index + offset);
        self.nodes.extend(inner.nodes);
        let inner_links: Vec<((usize, usize), Link)> = inner
            .links
            .into_iter()
            .map(|((node, socket), link)| {
                (
                    (node + offset, socket),
                    Link {
                        node: link.node + offset,
                        socket: link.socket,
                    },
                )
            })
            .collect();
        let slot_of = |sockets: &[Socket<'a>], identifier: &str, name: &str| {
            sockets
                .iter()
                .position(|socket| socket.identifier == identifier)
                .or_else(|| sockets.iter().position(|socket| socket.name == name))
        };
        for (target, source) in inner_links {
            if Some(source.node) != input_node {
                self.links.insert(target, source);
                continue;
            }
            let socket = &self.nodes[source.node].outputs[source.socket];
            let Some(slot) = slot_of(&self.nodes[group].inputs, &socket.identifier, &socket.name)
            else {
                continue;
            };
            match self.links.get(&(group, slot)).copied() {
                Some(outer) => {
                    self.links.insert(target, outer);
                }
                None => {
                    let outer = self.nodes[group].inputs[slot].record;
                    self.nodes[target.0].inputs[target.1].record = outer;
                }
            }
        }
        let Some(output_node) = output_node else {
            return;
        };
        let consumers: Vec<((usize, usize), Link)> = self
            .links
            .iter()
            .filter(|(_, link)| link.node == group)
            .map(|(target, link)| (*target, *link))
            .collect();
        for (target, link) in consumers {
            let socket = &self.nodes[group].outputs[link.socket];
            let Some(slot) = slot_of(
                &self.nodes[output_node].inputs,
                &socket.identifier,
                &socket.name,
            ) else {
                self.links.remove(&target);
                continue;
            };
            match self.links.get(&(output_node, slot)).copied() {
                Some(inner) => {
                    self.links.insert(target, inner);
                }
                None => {
                    self.links.remove(&target);
                    let inner = self.nodes[output_node].inputs[slot].record;
                    self.nodes[target.0].inputs[target.1].record = inner;
                }
            }
        }
    }

    pub(crate) fn linked(&self, node: usize, input: usize) -> Option<Link> {
        let mut link = *self.links.get(&(node, input))?;
        for _ in 0..64 {
            let source = &self.nodes[link.node];
            let passes = source.idname == "NodeReroute" || source.muted;
            if !passes {
                return Some(link);
            }
            let through = match source.idname.as_str() {
                "NodeReroute" => 0,
                _ => source
                    .inputs
                    .iter()
                    .position(|socket| socket.name == source.outputs[link.socket].name)
                    .unwrap_or(0),
            };
            link = *self.links.get(&(link.node, through))?;
        }
        None
    }

    pub(crate) fn output(&self) -> Option<usize> {
        let outputs: Vec<usize> = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.is("ShaderNodeOutputMaterial", 124))
            .map(|(index, _)| index)
            .collect();
        let active = |index: &&usize| {
            self.nodes[**index].record.int("flag").unwrap_or(0) & NODE_DO_OUTPUT != 0
        };
        let target = |index: &&usize| {
            self.nodes[**index].record.int("custom1").unwrap_or(0) != OUTPUT_CYCLES
        };
        outputs
            .iter()
            .find(|index| active(index) && target(index))
            .or_else(|| outputs.iter().find(target))
            .or_else(|| outputs.first())
            .copied()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Placement {
    pub(crate) offset: Vec2,
    pub(crate) scale: Vec2,
    pub(crate) rotation: f32,
}
