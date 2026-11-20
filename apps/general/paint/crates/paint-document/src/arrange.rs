use rikka_psd::{ChannelId, Compression, Document, Info, Node, NodePath, RgbaImage};

use crate::edit::Placement;
use crate::error::{Error, Result};
use crate::layer::{self, LayerId, located};

pub(crate) fn add(document: &mut Document, above: LayerId, name: &str) -> Result<LayerId> {
    let at = above_of(document, above)?;
    let mut record = empty(document, name)?;
    identify(document, &mut record);
    document.insert_node(&at, Node::Layer(record))?;
    layer::id_at(document, &at)
}

pub(crate) fn duplicate(document: &mut Document, id: LayerId, name: &str) -> Result<LayerId> {
    let mut copy = layer::node(document, id)
        .ok_or(Error::UnknownLayer(id))?
        .clone();
    if let Node::Layer(record) = &mut copy {
        transparent(document, record)?;
    }
    copy.layer_mut().set_name(name);
    let mut next = next_id(document);
    layer::records(&mut copy, &mut |record| {
        if record.id().is_some() {
            record.set_info(Info::Id(next));
            next += 1;
        }
    });
    let at = above_of(document, id)?;
    document.insert_node(&at, copy)?;
    layer::id_at(document, &at)
}

pub(crate) fn delete(document: &mut Document, id: LayerId) -> Result<LayerId> {
    let at = located(document, id)?;
    document.remove_node(&at).ok_or(Error::UnknownLayer(id))?;
    let left = document.siblings(&at).map_or(0, <[Node]>::len);
    let next_active = match (at.index(), left) {
        (_, 0) => at.group(),
        (0, _) => Some(at),
        (index, _) => Some(at.sibling(index - 1)),
    };
    match next_active {
        None => Err(Error::LastLayer),
        Some(next_active) => layer::id_at(document, &next_active),
    }
}

pub(crate) fn shift(
    document: &mut Document,
    id: LayerId,
    to: Placement,
) -> Result<Option<LayerId>> {
    let from = located(document, id)?;
    let target = destination(document, &from, to)?;
    if from.holds(&target) {
        return Err(Error::MoveIntoItself);
    }
    let moved = document.remove_node(&from).ok_or(Error::UnknownLayer(id))?;
    let target = target.after_removal(&from);
    if target == from {
        document.insert_node(&from, moved)?;
        return Ok(None);
    }
    document.insert_node(&target, moved)?;
    Ok(Some(layer::id_at(document, &target)?))
}

pub(crate) fn unbackground(document: &mut Document, name: &str) -> Result<LayerId> {
    let Some(Node::Layer(bottom)) = document.layers.first() else {
        return Err(Error::NoBackground);
    };
    if !layer::is_opaque_record(bottom) {
        return Err(Error::NoBackground);
    }
    let mut record = bottom.clone();
    transparent(document, &mut record)?;
    record.set_name(name);
    document.layers[0] = Node::Layer(record);
    Ok(LayerId(0))
}

pub(crate) fn empty(document: &Document, name: &str) -> Result<rikka_psd::Layer> {
    let mut record = rikka_psd::Layer::new(name);
    let nothing = RgbaImage {
        width: 0,
        height: 0,
        pixels: Vec::new(),
        linear: document.depth == rikka_psd::Depth::ThirtyTwo,
    };
    document.set_layer_rgba(&mut record, 0, 0, &nothing, Compression::Rle)?;
    Ok(record)
}

pub(crate) fn identify(document: &Document, record: &mut rikka_psd::Layer) {
    if document
        .layer_records()
        .iter()
        .any(|child| child.id().is_some())
    {
        record.set_info(Info::Id(next_id(document)));
    }
}

pub(crate) fn next_id(document: &Document) -> u32 {
    document
        .layer_records()
        .iter()
        .filter_map(|record| record.id())
        .max()
        .map_or(1, |most| most + 1)
}

fn transparent(document: &Document, record: &mut rikka_psd::Layer) -> Result<()> {
    if record.channel(ChannelId::Transparency).is_some() || record.bounds.is_empty() {
        return Ok(());
    }
    let pixels = document.layer_rgba(record)?;
    let bounds = record.bounds;
    document
        .set_layer_rgba(record, bounds.left, bounds.top, &pixels, Compression::Rle)
        .map_err(Error::from)
}

fn above_of(document: &Document, id: LayerId) -> Result<NodePath> {
    let at = located(document, id)?;
    Ok(at.sibling(at.index() + 1))
}

fn destination(document: &Document, from: &NodePath, to: Placement) -> Result<NodePath> {
    let index = from.index();
    let count = document.siblings(from).map_or(0, <[Node]>::len);
    Ok(match to {
        Placement::Above(target) => above_of(document, target)?,
        Placement::Below(target) => located(document, target)?,
        Placement::Into(target) => {
            let group = located(document, target)?;
            match document.node(&group) {
                Some(Node::Group(child_group)) => group.child(child_group.children.len()),
                Some(Node::Layer(_)) => return Err(Error::NotGroup),
                None => return Err(Error::UnknownLayer(target)),
            }
        }
        Placement::Forward if index + 1 < count => from.sibling(index + 2),
        Placement::Backward if index > 0 => from.sibling(index - 1),
        Placement::Forward => {
            let group = from.group().ok_or(Error::AtTop)?;
            group.sibling(group.index() + 1)
        }
        Placement::Backward => from.group().ok_or(Error::AtBottom)?,
    })
}
