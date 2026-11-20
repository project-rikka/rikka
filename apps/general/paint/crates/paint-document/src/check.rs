use rikka_psd::BlendMode;

use crate::edit::{Edit, FigureEdit, Placement};
use crate::error::{Error, Result};
use crate::flood::{Flood, Source};
use crate::layer::{Layer, LayerId, LayerKind};

struct Spot<'a> {
    layer: &'a Layer,
    siblings: &'a [Layer],
    at: usize,
    parent: Option<&'a Layer>,
}

impl Edit {
    pub fn check(&self, layers: &[Layer]) -> Result<()> {
        match self {
            Edit::NewLayer { above: layer, .. } | Edit::DuplicateLayer { layer, .. } => {
                spot(layers, *layer).map(|_| ())
            }
            Edit::RenameLayer { layer, .. } | Edit::SetOpacity { layer, .. } => {
                unlocked(layers, *layer).map(|_| ())
            }
            Edit::SetBlend { layer, blend } => blendable(unlocked(layers, *layer)?, *blend),
            Edit::SetProperties { layer, properties } => {
                blendable(unlocked(layers, *layer)?, properties.blend)
            }
            Edit::DeleteLayer { layer } => deletable(layers, *layer),
            Edit::MoveLayer { layer, to } => movable(layers, *layer, *to),
            Edit::MergeDown { layer } => mergeable(layers, *layer),
            Edit::MergeVisible => match shown_leaves(layers) >= 2 {
                true => Ok(()),
                false => Err(Error::TooFewVisible),
            },
            Edit::Flatten => match layers {
                [only] if only.background => Err(Error::AlreadyFlat),
                _ => Ok(()),
            },
            Edit::LayerFromBackground { .. } => match layers.last() {
                Some(bottom) if bottom.background => Ok(()),
                _ => Err(Error::NoBackground),
            },
            Edit::Bucket { layer, flood, .. } => {
                paintable(layers, *layer)?;
                sampleable(layers, flood)
            }
            Edit::Figures {
                layer,
                change: FigureEdit::Add { .. },
            } => paintable(layers, *layer),
            Edit::Figures { layer, .. } => pixels(layers, *layer).map(|_| ()),
            Edit::MagicWand { flood, .. } => sampleable(layers, flood),
            Edit::SelectLayer { layer, .. } => pixels(layers, *layer).map(|_| ()),
            _ => Ok(()),
        }
    }
}

fn unlocked(layers: &[Layer], id: LayerId) -> Result<&Layer> {
    let layer = spot(layers, id)?.layer;
    match layer.background {
        true => Err(Error::BackgroundLocked),
        false => Ok(layer),
    }
}

fn pixels(layers: &[Layer], id: LayerId) -> Result<&Layer> {
    let layer = spot(layers, id)?.layer;
    match layer.kind {
        LayerKind::Pixel => Ok(layer),
        _ => Err(Error::NotPixelLayer(id)),
    }
}

fn paintable(layers: &[Layer], id: LayerId) -> Result<()> {
    match pixels(layers, id)?.visible {
        true => Ok(()),
        false => Err(Error::HiddenLayer(id)),
    }
}

fn sampleable(layers: &[Layer], flood: &Flood) -> Result<()> {
    match flood.sampled() {
        Source::Image => Ok(()),
        Source::Layer(id) => pixels(layers, id).map(|_| ()),
    }
}

fn blendable(layer: &Layer, blend: BlendMode) -> Result<()> {
    match (blend, layer.kind) {
        (BlendMode::Unknown(_), _) => Err(Error::UnknownBlend),
        (BlendMode::PassThrough, kind) if kind != LayerKind::Group => {
            Err(Error::PassThroughOutsideGroup)
        }
        _ => Ok(()),
    }
}

fn deletable(layers: &[Layer], id: LayerId) -> Result<()> {
    let spot = spot(layers, id)?;
    match spot.parent.is_none() && spot.siblings.len() == 1 {
        true => Err(Error::LastLayer),
        false => Ok(()),
    }
}

fn movable(layers: &[Layer], id: LayerId, to: Placement) -> Result<()> {
    let moved = spot(layers, id)?;
    if moved.layer.background {
        return Err(Error::BackgroundLocked);
    }
    let inside_itself =
        |target: LayerId| Layer::find(std::slice::from_ref(moved.layer), target).is_some();
    match to {
        Placement::Above(target) | Placement::Below(target)
            if target != id && inside_itself(target) =>
        {
            Err(Error::MoveIntoItself)
        }
        Placement::Into(target) if inside_itself(target) => Err(Error::MoveIntoItself),
        Placement::Above(target) => spot(layers, target).map(|_| ()),
        Placement::Below(target) => match spot(layers, target)?.layer.background {
            true => Err(Error::BackgroundLocked),
            false => Ok(()),
        },
        Placement::Into(target) => match spot(layers, target)?.layer.kind {
            LayerKind::Group => Ok(()),
            _ => Err(Error::NotGroup),
        },
        Placement::Forward => match moved.at > 0 || moved.parent.is_some() {
            true => Ok(()),
            false => Err(Error::AtTop),
        },
        Placement::Backward => match moved.siblings.get(moved.at + 1) {
            Some(below) if below.background => Err(Error::BackgroundLocked),
            Some(_) => Ok(()),
            None if moved.parent.is_some() => Ok(()),
            None => Err(Error::AtBottom),
        },
    }
}

fn mergeable(layers: &[Layer], id: LayerId) -> Result<()> {
    let spot = spot(layers, id)?;
    if spot.layer.kind == LayerKind::Group {
        return Err(Error::GroupMerge);
    }
    match spot.siblings.get(spot.at + 1) {
        Some(below) if below.kind == LayerKind::Pixel => Ok(()),
        _ => Err(Error::NothingBelow),
    }
}

fn shown_leaves(layers: &[Layer]) -> usize {
    layers
        .iter()
        .filter(|layer| layer.visible)
        .map(|layer| match layer.kind {
            LayerKind::Group => shown_leaves(&layer.children),
            _ => 1,
        })
        .sum()
}

fn spot(layers: &[Layer], id: LayerId) -> Result<Spot<'_>> {
    fn walk<'a>(siblings: &'a [Layer], parent: Option<&'a Layer>, id: LayerId) -> Option<Spot<'a>> {
        siblings.iter().enumerate().find_map(|(at, layer)| {
            if layer.id == id {
                return Some(Spot {
                    layer,
                    siblings,
                    at,
                    parent,
                });
            }
            walk(&layer.children, Some(layer), id)
        })
    }
    walk(layers, None, id).ok_or_else(|| Error::UnknownLayer(id))
}
