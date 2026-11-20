use rikka_mmd::VmdCurve;
use rikka_renderer_3d::{Literal, ParameterKind};
use rikka_shader_graph::{Constant, Library, ParameterDecl, PortDefault, SocketValue};

use crate::error::Error;
use crate::graph_ref::SocketKey;
use crate::keys::{LookKey, SlotKey, Target};
use crate::look::{Look, LookEdit, port_of};
use crate::name::Name;
use crate::slots::SlotEdit;
use crate::value::{LookValue, Texture};

impl Look {
    #[must_use]
    pub fn slot_parameter(&self, key: &SlotKey) -> Option<&ParameterDecl> {
        let slot = self.slots.slot(key.figure, key.material)?;
        self.materials
            .get(slot.material.as_deref()?)?
            .parameter(key.parameter.as_str())
    }

    #[must_use]
    pub fn default_value(&self, library: &Library, target: &Target) -> Option<LookValue> {
        match target {
            Target::Socket(key) => {
                let port = port_of(self.graph(&key.graph)?, library, &key.socket)?;
                let PortDefault::Value(constant) = port.default else {
                    return None;
                };
                Some(LookValue::from_socket(
                    &SocketValue::Number(constant),
                    port.hint,
                ))
            }
            Target::Slot(key) => LookValue::default_of_parameter(self.slot_parameter(key)?),
        }
    }

    #[must_use]
    pub fn value_from_socket(
        &self,
        library: &Library,
        target: &Target,
        value: &SocketValue,
    ) -> Option<LookValue> {
        match target {
            Target::Socket(key) => Some(LookValue::from_socket(
                value,
                self.socket_hint(library, key)?,
            )),
            Target::Slot(key) => LookValue::from_parameter(&self.slot_parameter(key)?.kind, value),
        }
    }

    #[must_use]
    pub fn value_to_socket(&self, target: &Target, value: &LookValue) -> Option<SocketValue> {
        match target {
            Target::Socket(_) => value.to_socket(),
            Target::Slot(key) => value.to_parameter_socket(&self.slot_parameter(key)?.kind),
        }
    }

    #[must_use]
    pub fn is_keyed(&self, target: &Target) -> bool {
        !self.keys.tracks().keys(target).is_empty()
    }

    #[must_use]
    pub fn value_at(&self, library: &Library, target: &Target, frame: f32) -> Option<LookValue> {
        if let Some(value) = self.keys.value_at(target, frame) {
            return Some(value);
        }
        match target {
            Target::Socket(key) => {
                let hint = self.socket_hint(library, key)?;
                Some(LookValue::from_socket(self.socket(key)?, hint))
            }
            Target::Slot(key) => self
                .slots
                .slot(key.figure, key.material)?
                .values
                .get(&key.parameter)
                .cloned(),
        }
    }

    pub fn set_value(
        &mut self,
        library: &Library,
        target: &Target,
        value: Option<LookValue>,
        frame: u32,
        curve: VmdCurve,
    ) -> Result<LookEdit, Error> {
        if let Some(value) = &value
            && self.is_keyed(target)
        {
            return self.lay_key(library, target, value.clone(), frame, curve);
        }
        let edit = match target {
            Target::Socket(key) => {
                self.socket_hint(library, key).ok_or_else(|| Error::Key {
                    target: key.to_string(),
                    reason: "the graph has no such input".into(),
                })?;
                let socket = match value {
                    Some(value) => Some(socket_of(&value, key)?),
                    None => None,
                };
                LookEdit::Graph {
                    graph: key.graph.clone(),
                    edit: key.edit(socket),
                }
            }
            Target::Slot(key) => LookEdit::Slots(self.slot_edit(*key, value)),
        };
        self.apply(library, edit)
    }

    pub fn toggle_key(
        &mut self,
        library: &Library,
        target: &Target,
        value: LookValue,
        frame: u32,
        curve: VmdCurve,
    ) -> Result<LookEdit, Error> {
        match self.keys.tracks().key(target, frame) {
            Some(_) => {
                let removing = self.keys.removing(target, frame);
                self.apply(library, LookEdit::Keys(removing))
            }
            None => self.lay_key(library, target, value, frame, curve),
        }
    }

    fn lay_key(
        &mut self,
        library: &Library,
        target: &Target,
        value: LookValue,
        frame: u32,
        curve: VmdCurve,
    ) -> Result<LookEdit, Error> {
        let curve = self
            .keys
            .tracks()
            .key(target, frame)
            .map_or(curve, |key| key.curve);
        match target {
            Target::Socket(key) => {
                let socket = socket_of(&value, key)?;
                self.key_socket(library, key.clone(), frame, socket, curve)
            }
            Target::Slot(_) => {
                let inserting = self.keys.inserting(LookKey {
                    target: target.clone(),
                    frame,
                    value,
                    curve,
                });
                self.apply(library, LookEdit::Keys(inserting))
            }
        }
    }

    fn slot_edit(&self, key: SlotKey, value: Option<LookValue>) -> SlotEdit {
        let mut slot = self
            .slots
            .slot(key.figure, key.material)
            .cloned()
            .unwrap_or_default();
        match value {
            Some(value) => slot.values.insert(key.parameter, value),
            None => slot.values.remove(&key.parameter),
        };
        SlotEdit::Replace {
            figure: key.figure,
            material: key.material,
            slot: Some(slot),
        }
    }
}

fn socket_of(value: &LookValue, key: &SocketKey) -> Result<SocketValue, Error> {
    value.to_socket().ok_or_else(|| Error::Key {
        target: key.to_string(),
        reason: format!("a {} does not fit a socket", value.kind_name()),
    })
}

impl LookValue {
    #[must_use]
    pub fn from_parameter(kind: &ParameterKind, value: &SocketValue) -> Option<Self> {
        match (kind, value) {
            (ParameterKind::Texture { kind, .. }, SocketValue::Asset(path)) => {
                Some(Self::Texture(Some(Texture::new(path, *kind))))
            }
            (ParameterKind::Texture { .. }, SocketValue::Number(_))
            | (_, SocketValue::Asset(_)) => None,
            (kind, SocketValue::Number(number)) => {
                let at = |index: usize| number.resized(4).component(index);
                let first = number.component(0);
                Some(match kind {
                    ParameterKind::Float => Self::Float(first),
                    ParameterKind::Vec2 => Self::Vec2([at(0), at(1)]),
                    ParameterKind::Vec3 | ParameterKind::Color { alpha: false, .. } => {
                        Self::Vec3([at(0), at(1), at(2)])
                    }
                    ParameterKind::Vec4 | ParameterKind::Color { alpha: true, .. } => {
                        Self::Vec4([at(0), at(1), at(2), at(3)])
                    }
                    ParameterKind::Int => Self::Int(first.round() as i32),
                    ParameterKind::UInt => Self::UInt(first.round().max(0.0) as u32),
                    ParameterKind::Bool => Self::Bool(first > 0.5),
                    ParameterKind::Enum { options } => {
                        let last = options.len().checked_sub(1)?;
                        let chosen = (first.round().max(0.0) as usize).min(last);
                        Self::Enum(Name::new(&options[chosen]))
                    }
                    ParameterKind::Texture { .. } => return None,
                })
            }
        }
    }

    #[must_use]
    pub fn to_parameter_socket(&self, kind: &ParameterKind) -> Option<SocketValue> {
        match (self, kind) {
            (Self::Texture(texture), _) => {
                Some(SocketValue::Asset(texture.as_ref()?.asset.to_string()))
            }
            (Self::Enum(option), ParameterKind::Enum { options }) => {
                let index = options.iter().position(|known| known == option.as_str())?;
                Some(SocketValue::from(index as f32))
            }
            (held, _) => held.to_socket(),
        }
    }

    #[must_use]
    pub fn default_of_parameter(decl: &ParameterDecl) -> Option<Self> {
        let literal = decl.default.unwrap_or(match decl.kind {
            ParameterKind::Texture { .. } => Literal::Texture,
            _ => Literal::Float(0.0),
        });
        let socket = match literal {
            Literal::Texture => return Some(Self::Texture(None)),
            Literal::Float(value) => Constant::scalar(value),
            Literal::Vec2([x, y]) => Constant::vec2(x, y),
            Literal::Vec3([x, y, z]) => Constant::vec3(x, y, z),
            Literal::Vec4([x, y, z, w]) => Constant::vec4(x, y, z, w),
            Literal::Int(value) => Constant::scalar(value as f32),
            Literal::UInt(value) | Literal::Enum(value) => Constant::scalar(value as f32),
            Literal::Bool(on) => Constant::scalar(if on { 1.0 } else { 0.0 }),
        };
        Self::from_parameter(&decl.kind, &SocketValue::Number(socket))
    }
}
