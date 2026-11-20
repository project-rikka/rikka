use paint_document::{Modify, Shape};
use serde::Deserialize;
use serde_json::{Map, Value as Json};

use super::draw::flood;
use super::item::{Item, changed, numbered, sized};
use super::read;
use crate::ops::Parsed;
use crate::order::{Change, Order, SelectStep};
use crate::svg;
use crate::vocabulary::{COMBINES, FILL_RULES, SELECTIONS};

const MAX_MODIFY: f32 = 1000.0;
const MAX_POLYGON: usize = 10_000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectArgs {
    #[serde(default)]
    document: Option<String>,
    name: String,
    steps: Vec<Map<String, Json>>,
}

pub(crate) fn select(input: Json) -> Parsed<Order> {
    let args: SelectArgs = read(input)?;
    let steps = numbered(args.steps, "step", step)?;
    changed(args.document, args.name, Change::Select(steps))
}

fn step(fields: Map<String, Json>) -> Parsed<SelectStep> {
    let mut item = Item::read(fields, "op", &SELECTIONS, "op")?;
    let combine = item.named("combine", &COMBINES)?.unwrap_or_default();
    let shaped = |shape: Shape, item: &mut Item| -> Parsed<SelectStep> {
        Ok(SelectStep::Shape {
            shape: shape.anti_aliased(item.take("antialias")?.unwrap_or(true)),
            combine,
        })
    };
    Ok(match item.kind() {
        "rectangle" | "ellipse" => {
            let [x, y, width, height] = sized(item.need("rect", "[x, y, width, height]")?, "rect")?;
            let shape = match item.kind() {
                "rectangle" => Shape::rectangle(x, y, width, height),
                _ => Shape::ellipse(x, y, width, height),
            };
            shaped(shape, &mut item)?
        }
        "polygon" => {
            let points: Vec<[f32; 2]> = item.need("points", "[[x, y], …], three or more")?;
            if points.len() < 3 || points.len() > MAX_POLYGON {
                return Err(format!(
                    "a polygon takes 3–{MAX_POLYGON} points, not {}",
                    points.len()
                ));
            }
            let outline = points.into_iter().map(|[x, y]| (x, y));
            shaped(Shape::polygon([outline], Default::default()), &mut item)?
        }
        "path" => {
            let d: String = item.need("d", "SVG path data")?;
            let rule = item.named("fill_rule", &FILL_RULES)?.unwrap_or_default();
            shaped(
                Shape::polygon(svg::outlines(&svg::path(&d)?), rule),
                &mut item,
            )?
        }
        "wand" => SelectStep::Wand {
            flood: flood(&mut item)?,
            combine,
        },
        "layer" => SelectStep::Layer {
            layer: item.layer("layer")?,
            combine,
        },
        "all" => SelectStep::All,
        "none" => SelectStep::Nothing,
        "invert" => SelectStep::Invert,
        "move" => SelectStep::Move {
            dx: item.take("dx")?.unwrap_or(0),
            dy: item.take("dy")?.unwrap_or(0),
        },
        modify => {
            let by = item
                .number_in_range("by", 0.0, MAX_MODIFY)?
                .ok_or_else(|| format!("{modify} needs `by`, in px"))?;
            let whole = by.round() as u32;
            SelectStep::Modify(match modify {
                "expand" => Modify::Expand(whole),
                "contract" => Modify::Contract(whole),
                "feather" => Modify::Feather(by),
                "border" => Modify::Border(whole.max(1)),
                _ => Modify::Smooth(whole),
            })
        }
    })
}
