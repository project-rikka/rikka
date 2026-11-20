use std::collections::BTreeMap;

use rikka_renderer_3d::{TextureKind, Value};
use rikka_shader_compile::socket_value;
use rikka_shader_graph::{Socket, SocketValue};

use crate::asset::Asset;
use crate::graph_ref::GraphRef;
use crate::name::Name;
use crate::plans::Evaluated;
use crate::value::LookValue;

#[derive(Clone, Debug, PartialEq)]
pub enum MaterialValue {
    Value(Value),
    Texture { path: Asset, kind: TextureKind },
}

impl Evaluated<'_> {
    #[must_use]
    pub fn material_values(
        &self,
        figure: Name,
        material: u32,
        key: &str,
        sockets: &[Socket],
    ) -> Vec<(String, MaterialValue)> {
        let graph = GraphRef::Material(key.to_owned());
        let mut values = Vec::with_capacity(sockets.len());
        for socket in sockets {
            let value = match self.socket(&graph, &socket.at) {
                Some(SocketValue::Number(constant)) => {
                    socket_value(socket.ty, constant).map(MaterialValue::Value)
                }
                Some(SocketValue::Asset(path)) => Some(MaterialValue::Texture {
                    path: Asset::new(&path),
                    kind: socket.texture.unwrap_or(TextureKind::Colour),
                }),
                None => None,
            };
            if let Some(value) = value {
                values.push((socket.name.clone(), value));
            }
        }
        let Some(held) = self.look().graph(&graph) else {
            return values;
        };
        let mut instance: BTreeMap<&str, &LookValue> = BTreeMap::new();
        if let Some(slot) = self.look().slots.slot(figure, material) {
            for (name, value) in &slot.values {
                instance.insert(name.as_str(), value);
            }
        }
        for (name, value) in self.sampled.slot_values(figure, material) {
            instance.insert(name.as_str(), value);
        }
        for (name, value) in instance {
            if held.parameter(name).is_none() {
                continue;
            }
            let value = match value {
                LookValue::Texture(Some(texture)) => MaterialValue::Texture {
                    path: texture.asset.clone(),
                    kind: texture.kind,
                },
                held => MaterialValue::Value(held.to_setting(|_| None)),
            };
            values.push((name.to_owned(), value));
        }
        values
    }
}
