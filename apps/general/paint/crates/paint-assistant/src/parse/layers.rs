use serde::Deserialize;
use serde_json::{Map, Value as Json};

use super::item::{Item, changed, numbered};
use super::read;
use crate::ops::Parsed;
use crate::order::{Change, LayerLook, LayerStep, Moving, Order};
use crate::vocabulary::{BLENDS, LAYERINGS};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LayersArgs {
    #[serde(default)]
    document: Option<String>,
    name: String,
    steps: Vec<Map<String, Json>>,
}

pub(crate) fn layers(input: Json) -> Parsed<Order> {
    let args: LayersArgs = read(input)?;
    let steps = numbered(args.steps, "step", step)?;
    changed(args.document, args.name, Change::Layers(steps))
}

fn named(item: &mut Item) -> Parsed<Option<String>> {
    match item.take::<String>("name")? {
        Some(name) if name.trim().is_empty() => Err("`name` is empty".to_owned()),
        other => Ok(other.map(|name| name.trim().to_owned())),
    }
}

fn step(fields: Map<String, Json>) -> Parsed<LayerStep> {
    let mut item = Item::read(fields, "op", &LAYERINGS, "op")?;
    let layer = item.layer("layer")?;
    Ok(match item.kind() {
        "new" => LayerStep::New {
            name: named(&mut item)?.ok_or("new needs `name`")?,
            above: item.layer("above")?,
        },
        "duplicate" => LayerStep::Duplicate {
            layer,
            name: named(&mut item)?,
        },
        "delete" => LayerStep::Delete { layer },
        "move" => LayerStep::Move {
            layer,
            to: moving(&mut item)?,
        },
        "merge_down" => LayerStep::MergeDown { layer },
        "merge_visible" => LayerStep::MergeVisible,
        "flatten" => LayerStep::Flatten,
        "from_background" => LayerStep::FromBackground {
            name: named(&mut item)?,
        },
        _ => {
            let look = LayerLook {
                name: named(&mut item)?,
                visible: item.take("visible")?,
                opacity: item.percent("opacity")?,
                blend: item.named("blend", &BLENDS)?,
            };
            let provided = [
                look.name.is_some(),
                look.visible.is_some(),
                look.opacity.is_some(),
                look.blend.is_some(),
            ];
            if !provided.contains(&true) {
                return Err("set needs `name`, `visible`, `opacity` or `blend`".to_owned());
            }
            LayerStep::Restyle { layer, look }
        }
    })
}

fn moving(item: &mut Item) -> Parsed<Moving> {
    let places = [
        item.layer("above")?.map(Moving::Above),
        item.layer("below")?.map(Moving::Below),
        item.layer("into")?.map(Moving::Into),
        steps(item)?.map(Moving::By),
    ];
    let mut provided = places.into_iter().flatten();
    match (provided.next(), provided.next()) {
        (Some(to), None) => Ok(to),
        _ => Err(
            "move takes one of `above`, `below`, `into` (a layer) or `by` (steps, positive up)"
                .to_owned(),
        ),
    }
}

fn steps(item: &mut Item) -> Parsed<Option<i32>> {
    match item.take::<f32>("by")? {
        Some(by) if by == 0.0 || by.fract() != 0.0 => {
            Err(format!("`by` is {by}; give whole steps, positive up"))
        }
        by => Ok(by.map(|by| by as i32)),
    }
}
