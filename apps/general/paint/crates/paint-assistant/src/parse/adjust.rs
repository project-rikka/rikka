use serde::Deserialize;
use serde_json::{Map, Value as Json};

use super::item::{changed, layer_ref, numbered, sized};
use super::read;
use crate::masking::{MaskOrder, MaskShape};
use crate::ops::{self, Parsed};
use crate::order::{AdjustStep, Change, Order};
use crate::svg;
use crate::vocabulary::{FILL_RULES, named};

const MAX_FEATHER: f32 = 1000.0;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AdjustArgs {
    #[serde(default)]
    document: Option<String>,
    name: String,
    steps: Vec<Map<String, Json>>,
}

pub(crate) fn adjust(input: Json) -> Parsed<Order> {
    let args: AdjustArgs = read(input)?;
    let adjusted = numbered(args.steps, "step", adjust_step)?;
    changed(args.document, args.name, Change::Adjust(adjusted))
}

fn adjust_step(mut step: Map<String, Json>) -> Parsed<AdjustStep> {
    let op = match step.remove("op") {
        Some(Json::String(op)) => op,
        _ => return Err("each step names its `op`".to_owned()),
    };
    let layer = match step.remove("layer") {
        None | Some(Json::Null) => None,
        Some(value) => Some(layer_ref(&value)?),
    };
    let mask = step.remove("mask").map(mask).transpose()?;
    Ok(AdjustStep {
        recipe: ops::adjustment(&op, step)?,
        layer,
        mask,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MaskArgs {
    #[serde(default)]
    rect: Option<[f32; 4]>,
    #[serde(default)]
    ellipse: Option<[f32; 4]>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    fill_rule: Option<String>,
    #[serde(default)]
    feather: f32,
    #[serde(default)]
    invert: bool,
}

fn mask(value: Json) -> Parsed<MaskOrder> {
    let args: MaskArgs =
        serde_json::from_value(value).map_err(|error| format!("invalid mask: {error}"))?;
    let shape = match (args.rect, args.ellipse, args.path) {
        (Some(area), None, None) => MaskShape::Rectangle(sized(area, "the mask rectangle")?),
        (None, Some(area), None) => MaskShape::Ellipse(sized(area, "the mask ellipse")?),
        (None, None, Some(d)) => {
            let path = svg::path(&d).map_err(|error| format!("the mask path: {error}"))?;
            let rule = args
                .fill_rule
                .as_deref()
                .map(|rule| named(&FILL_RULES, rule, "fill rule"))
                .transpose()?
                .unwrap_or_default();
            MaskShape::Path(path, rule)
        }
        _ => return Err("a mask is exactly one of `rect`, `ellipse` or `path`".to_owned()),
    };
    if !(0.0..=MAX_FEATHER).contains(&args.feather) {
        return Err(format!(
            "the mask feather is {}; use 0–{MAX_FEATHER} px",
            args.feather
        ));
    }
    Ok(MaskOrder {
        shape,
        feather: args.feather,
        invert: args.invert,
    })
}
