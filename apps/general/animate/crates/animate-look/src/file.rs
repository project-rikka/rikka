use std::collections::BTreeMap;
use std::sync::Arc;

use rikka_mmd::VmdCurve;
use rikka_shader_graph::{Graph, Library, PortType, SocketValue, ValueType};
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::graph_ref::{GraphRef, check_material_key};
use crate::keys::{KeyEdit, Keys, LookKey, Target};
use crate::look::{Look, check_kind, port_of};
use crate::name::Name;
use crate::slots::{Slot, SlotEdit, Slots};
use crate::value::LookValue;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LookFile {
    world: Graph,
    composite: Graph,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    materials: BTreeMap<String, Graph>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    slots: Vec<SlotFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    keys: Vec<KeyFile>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SlotFile {
    figure: Name,
    material: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    graph: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    values: BTreeMap<Name, LookValue>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct KeyFile {
    target: Target,
    frame: u32,
    value: LookValue,
    curve: [u8; 4],
}

impl LookFile {
    #[must_use]
    pub fn from_look(look: &Look) -> Self {
        Self {
            world: Graph::clone(&look.world),
            composite: Graph::clone(&look.composite),
            materials: look
                .materials
                .iter()
                .map(|(key, graph)| (key.clone(), Graph::clone(graph)))
                .collect(),
            slots: look
                .slots
                .iter()
                .map(|(figure, material, slot)| SlotFile {
                    figure,
                    material,
                    graph: slot.material.clone(),
                    values: slot.values.clone(),
                })
                .collect(),
            keys: look
                .keys
                .tracks()
                .all_keys()
                .map(|key| KeyFile {
                    target: key.target.clone(),
                    frame: key.frame,
                    value: key.value.clone(),
                    curve: key.curve.to_bytes(),
                })
                .collect(),
        }
    }

    pub fn look(&self, library: &Library) -> Result<Look, Error> {
        let graph = |at: GraphRef, held: &Graph| -> Result<Arc<Graph>, Error> {
            if let Some(key) = at.material() {
                check_material_key(key)?;
            }
            check_kind(&at, held)?;
            let report = held.validate(library);
            if !report.is_valid() {
                return Err(Error::Graph {
                    graph: at,
                    source: rikka_shader_graph::Error(report.errors().to_vec()),
                });
            }
            Ok(Arc::new(held.clone()))
        };
        let mut look = Look {
            world: graph(GraphRef::World, &self.world)?,
            composite: graph(GraphRef::Composite, &self.composite)?,
            materials: self
                .materials
                .iter()
                .map(|(key, held)| Ok((key.clone(), graph(GraphRef::Material(key.clone()), held)?)))
                .collect::<Result<_, Error>>()?,
            slots: Slots::default(),
            keys: Keys::default(),
        };
        let mut slots = Vec::new();
        for file in &self.slots {
            if look.slots.slot(file.figure, file.material).is_some()
                || slots.iter().any(|edit| {
                    matches!(edit, SlotEdit::Replace { figure, material, .. }
                        if *figure == file.figure && *material == file.material)
                })
            {
                return Err(Error::DuplicateSlot {
                    figure: file.figure.to_string(),
                    material: file.material,
                });
            }
            slots.push(SlotEdit::Replace {
                figure: file.figure,
                material: file.material,
                slot: Some(Slot {
                    material: file.graph.clone(),
                    values: file.values.clone(),
                }),
            });
        }
        look.slots.apply(SlotEdit::Batch(slots));
        let mut keys = KeyEdit::default();
        for file in &self.keys {
            check_key(&look, library, file)?;
            if keys
                .added
                .iter()
                .any(|key| key.target == file.target && key.frame == file.frame)
            {
                return Err(Error::DuplicateKey {
                    target: file.target.to_string(),
                    frame: file.frame,
                });
            }
            keys.added.push(LookKey {
                target: file.target.clone(),
                frame: file.frame,
                value: file.value.clone(),
                curve: VmdCurve::from_bytes(file.curve),
            });
        }
        look.keys.apply(keys);
        Ok(look)
    }
}

fn check_key(look: &Look, library: &Library, file: &KeyFile) -> Result<(), Error> {
    let Target::Socket(key) = &file.target else {
        return Ok(());
    };
    let refuse = |reason: &str| Error::Key {
        target: key.to_string(),
        reason: reason.to_owned(),
    };
    let graph = look
        .graph(&key.graph)
        .ok_or_else(|| refuse("its graph does not exist"))?;
    let port = port_of(graph, library, &key.socket)
        .ok_or_else(|| refuse("its graph has no such input"))?;
    let takes_files = port.ty == PortType::Value(ValueType::Texture);
    let fits = match file.value.to_socket() {
        Some(SocketValue::Asset(_)) => takes_files,
        Some(SocketValue::Number(_)) => !takes_files,
        None => false,
    };
    match fits {
        true => Ok(()),
        false => Err(refuse(&format!(
            "a {} does not fit the input",
            file.value.kind_name()
        ))),
    }
}
