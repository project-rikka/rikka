use rikka_psd::{BlendMode, Node};

use crate::edit::{Edit, Properties};
use crate::history::Action;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Restyle {
    Name(String),
    Opacity(f32),
    Blend(BlendMode),
    Properties(Properties),
}

impl Restyle {
    pub(crate) fn from_edit(edit: &Edit) -> Option<Restyle> {
        Some(match edit {
            Edit::RenameLayer { name, .. } => Restyle::Name(name.clone()),
            Edit::SetOpacity { opacity, .. } => Restyle::Opacity(*opacity),
            Edit::SetBlend { blend, .. } => Restyle::Blend(*blend),
            Edit::SetProperties { properties, .. } => Restyle::Properties(properties.clone()),
            _ => return None,
        })
    }

    pub(crate) fn action(&self) -> Action {
        match self {
            Restyle::Name(_) => Action::RenameLayer,
            Restyle::Opacity(_) => Action::LayerOpacity,
            Restyle::Blend(_) => Action::LayerBlend,
            Restyle::Properties(_) => Action::LayerProperties,
        }
    }

    pub(crate) fn apply(&self, node: &mut Node) -> bool {
        match self {
            Restyle::Name(name) => rename(node, name),
            Restyle::Opacity(opacity) => fade(node, *opacity),
            Restyle::Blend(blend) => blend_with(node, *blend),
            Restyle::Properties(properties) => {
                let named = rename(node, &properties.name);
                let visibility_changed = show(node, properties.visible);
                let faded = fade(node, properties.opacity);
                let blended = blend_with(node, properties.blend);
                named || visibility_changed || faded || blended
            }
        }
    }
}

fn rename(node: &mut Node, name: &str) -> bool {
    let record = node.layer_mut();
    let changed = record.name() != name;
    if changed {
        record.set_name(name);
    }
    changed
}

fn show(node: &mut Node, visible: bool) -> bool {
    let record = node.layer_mut();
    let changed = record.visible() != visible;
    record.set_visible(visible);
    changed
}

fn fade(node: &mut Node, opacity: f32) -> bool {
    let record = node.layer_mut();
    let stored = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
    let changed = record.opacity != stored;
    record.opacity = stored;
    changed
}

fn blend_with(node: &mut Node, blend: BlendMode) -> bool {
    let changed = node.blend_mode() != blend;
    if changed {
        match node {
            Node::Group(group) => group.set_blend_mode(blend),
            Node::Layer(record) => record.blend_mode = blend,
        }
    }
    changed
}
