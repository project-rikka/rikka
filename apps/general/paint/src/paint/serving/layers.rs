use std::fmt;

use paint_assistant::{LayerRef, blend_name};
use paint_document::{Layer, LayerKind};

pub(super) fn every_layer(layers: &[Layer]) -> Vec<&Layer> {
    let mut all = Vec::new();
    for layer in layers {
        all.push(layer);
        all.extend(every_layer(&layer.children));
    }
    all
}

pub(super) fn shown_name(layer: &Layer) -> String {
    match layer.name.is_empty() {
        true => "Background".to_owned(),
        false => layer.name.clone(),
    }
}

pub(super) fn kind_name(kind: LayerKind) -> &'static str {
    match kind {
        LayerKind::Pixel => "pixel",
        LayerKind::Text => "text",
        LayerKind::Shape => "shape",
        LayerKind::Fill => "fill",
        LayerKind::Adjustment => "adjustment",
        LayerKind::SmartObject => "smart object",
        LayerKind::Group => "group",
    }
}

pub(super) fn listing<'a>(layers: impl IntoIterator<Item = &'a Layer>) -> String {
    layers
        .into_iter()
        .map(|layer| format!("{} \"{}\"", layer.id.index(), shown_name(layer)))
        .collect::<Vec<String>>()
        .join(", ")
}

pub(super) fn find<'a>(layers: &'a [Layer], requested: &LayerRef) -> Result<&'a Layer, String> {
    let all = every_layer(layers);
    let matches: Vec<&Layer> = all
        .iter()
        .copied()
        .filter(|layer| match requested {
            LayerRef::Index(index) => layer.id.index() == *index,
            LayerRef::Name(name) => shown_name(layer) == *name,
        })
        .collect();
    match (matches.as_slice(), requested) {
        ([only], _) => Ok(only),
        ([], LayerRef::Index(index)) => Err(format!(
            "there is no layer {index}; the layers are {}",
            listing(all)
        )),
        ([], LayerRef::Name(name)) => Err(format!(
            "there is no layer \"{name}\"; the layers are {}",
            listing(all)
        )),
        (several, _) => Err(format!(
            "several layers are called that: {}; give the index",
            listing(several.iter().copied())
        )),
    }
}

pub(super) fn paintable(layers: &[Layer], layer: &Layer) -> Result<(), String> {
    let (index, name) = (layer.id.index(), shown_name(layer));
    if layer.kind != LayerKind::Pixel {
        return Err(format!(
            "layer {index} \"{name}\" is a {} layer; only pixel layers change: {}",
            kind_name(layer.kind),
            listing(
                every_layer(layers)
                    .into_iter()
                    .filter(|layer| layer.kind == LayerKind::Pixel)
            )
        ));
    }
    if !layer.visible {
        return Err(format!(
            "layer {index} \"{name}\" is hidden; show it with the Layers tool's `set` `visible` first or work on a visible layer"
        ));
    }
    Ok(())
}

pub(super) struct LayerTree<'a>(pub(super) &'a [Layer]);

impl fmt::Display for LayerTree<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_tree(f, self.0, 1)
    }
}

fn write_tree(out: &mut impl fmt::Write, layers: &[Layer], depth: usize) -> fmt::Result {
    for layer in layers {
        write!(
            out,
            "{}{} \"{}\" {}",
            "  ".repeat(depth),
            layer.id.index(),
            shown_name(layer),
            kind_name(layer.kind)
        )?;
        if layer.background {
            out.write_str(", Background")?;
        }
        if !layer.visible {
            out.write_str(", hidden")?;
        }
        if layer.opacity < 0.999 {
            write!(out, ", {:.0}%", layer.opacity * 100.0)?;
        }
        let blend = blend_name(layer.blend);
        if blend != "normal" && blend != "pass_through" {
            write!(out, ", {blend}")?;
        }
        if layer.clipped {
            out.write_str(", clipped")?;
        }
        out.write_char('\n')?;
        for figure in layer.figures.iter().rev() {
            writeln!(
                out,
                "{}object {}",
                "  ".repeat(depth + 1),
                super::objects::describe(figure)
            )?;
        }
        write_tree(out, &layer.children, depth + 1)?;
    }
    Ok(())
}
