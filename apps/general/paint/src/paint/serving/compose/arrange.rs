use paint_assistant::{LayerLook, LayerStep, Moving};
use paint_document::{Edit, Layer, LayerId, Placement, Properties};

use super::{Composer, Refusal, layers};

const FROM_BACKGROUND: &str = "Layer 0";

impl Composer<'_> {
    pub(super) fn arrange(&mut self, steps: Vec<LayerStep>) -> Result<(), Refusal> {
        for (at, step) in steps.into_iter().enumerate() {
            let kind = kind(&step);
            let arranged = self.arranged(step);
            if let Err(Refusal(reason)) = arranged {
                return Err(Refusal(format!(
                    "step {} ({kind}): {reason}; the layers then were {}",
                    at + 1,
                    layers::listing(layers::every_layer(&self.document.layers()))
                )));
            }
        }
        Ok(())
    }

    fn arranged(&mut self, step: LayerStep) -> Result<(), Refusal> {
        let edit = match step {
            LayerStep::New { name, above } => Edit::NewLayer {
                above: self.layer(above.as_ref())?.id,
                name,
            },
            LayerStep::Duplicate { layer, name } => {
                let layer = self.layer(layer.as_ref())?;
                Edit::DuplicateLayer {
                    layer: layer.id,
                    name: name.unwrap_or_else(|| format!("{} copy", layers::shown_name(&layer))),
                }
            }
            LayerStep::Delete { layer } => Edit::DeleteLayer {
                layer: self.layer(layer.as_ref())?.id,
            },
            LayerStep::Move { layer, to } => {
                let layer = self.layer(layer.as_ref())?.id;
                let to = match to {
                    Moving::Above(target) => Placement::Above(self.layer(Some(&target))?.id),
                    Moving::Below(target) => Placement::Below(self.layer(Some(&target))?.id),
                    Moving::Into(target) => Placement::Into(self.layer(Some(&target))?.id),
                    Moving::By(steps) => return self.shift(layer, steps),
                };
                Edit::MoveLayer { layer, to }
            }
            LayerStep::Restyle { layer, look } => restyled(&self.layer(layer.as_ref())?, look),
            LayerStep::MergeDown { layer } => Edit::MergeDown {
                layer: self.layer(layer.as_ref())?.id,
            },
            LayerStep::MergeVisible => Edit::MergeVisible,
            LayerStep::Flatten => Edit::Flatten,
            LayerStep::FromBackground { name } => Edit::LayerFromBackground {
                name: name.unwrap_or_else(|| FROM_BACKGROUND.to_owned()),
            },
        };
        self.apply(&edit)
    }

    fn shift(&mut self, layer: LayerId, steps: i32) -> Result<(), Refusal> {
        let layers = self.document.layers();
        let siblings = siblings(&layers, layer).unwrap_or_default();
        let at = siblings
            .iter()
            .position(|sibling| sibling.id == layer)
            .unwrap_or_default();
        let room = match steps > 0 {
            true => at,
            false => siblings.len().saturating_sub(at + 1),
        };
        let reach = steps.unsigned_abs() as usize;
        if reach > room {
            let way = if steps > 0 { "up" } else { "down" };
            return Err(Refusal(format!(
                "the layer can move at most {room} {way} among its siblings"
            )));
        }
        let to = match steps > 0 {
            true => Placement::Above(siblings[at - reach].id),
            false => Placement::Below(siblings[at + reach].id),
        };
        self.apply(&Edit::MoveLayer { layer, to })
    }
}

fn siblings(layers: &[Layer], id: LayerId) -> Option<&[Layer]> {
    match layers.iter().any(|layer| layer.id == id) {
        true => Some(layers),
        false => layers
            .iter()
            .find_map(|layer| siblings(&layer.children, id)),
    }
}

fn restyled(layer: &Layer, look: LayerLook) -> Edit {
    let id = layer.id;
    match look {
        LayerLook {
            name: Some(name),
            visible: None,
            opacity: None,
            blend: None,
        } => Edit::RenameLayer { layer: id, name },
        LayerLook {
            name: None,
            visible: Some(visible),
            opacity: None,
            blend: None,
        } => Edit::Show { layer: id, visible },
        LayerLook {
            name: None,
            visible: None,
            opacity: Some(opacity),
            blend: None,
        } => Edit::SetOpacity { layer: id, opacity },
        LayerLook {
            name: None,
            visible: None,
            opacity: None,
            blend: Some(blend),
        } => Edit::SetBlend { layer: id, blend },
        LayerLook {
            name,
            visible,
            opacity,
            blend,
        } => Edit::SetProperties {
            layer: id,
            properties: Properties {
                name: name.unwrap_or_else(|| layer.name.clone()),
                visible: visible.unwrap_or(layer.visible),
                opacity: opacity.unwrap_or(layer.opacity),
                blend: blend.unwrap_or(layer.blend),
            },
        },
    }
}

fn kind(step: &LayerStep) -> &'static str {
    match step {
        LayerStep::New { .. } => "new",
        LayerStep::Duplicate { .. } => "duplicate",
        LayerStep::Delete { .. } => "delete",
        LayerStep::Move { .. } => "move",
        LayerStep::Restyle { .. } => "set",
        LayerStep::MergeDown { .. } => "merge_down",
        LayerStep::MergeVisible => "merge_visible",
        LayerStep::Flatten => "flatten",
        LayerStep::FromBackground { .. } => "from_background",
    }
}
