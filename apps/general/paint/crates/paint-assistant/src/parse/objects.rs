use rikka_types::Point;
use serde::Deserialize;
use serde_json::{Map, Value as Json};

use super::draw::{drawn, point};
use super::item::{Item, changed, numbered};
use super::read;
use crate::ops::Parsed;
use crate::order::{Change, ObjectStep, Order};
use crate::vocabulary::{OBJECT_ITEMS, OBJECTINGS, STACKINGS, listed};

const MAX_OBJECTS: usize = 256;
const STEP: f32 = 10.0;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ObjectsArgs {
    #[serde(default)]
    document: Option<String>,
    name: String,
    steps: Vec<Map<String, Json>>,
}

pub(crate) fn objects(input: Json) -> Parsed<Order> {
    let args: ObjectsArgs = read(input)?;
    let steps = numbered(args.steps, "step", step)?;
    changed(args.document, args.name, Change::Objects(steps))
}

fn step(fields: Map<String, Json>) -> Parsed<ObjectStep> {
    let mut item = Item::read(fields, "op", &OBJECTINGS, "op")?;
    Ok(match item.kind() {
        "move" => ObjectStep::Move {
            objects: ids(&mut item)?,
            offset: offset(&mut item)?.ok_or("move needs `offset` [dx, dy]")?,
        },
        "rotate" => ObjectStep::Rotate {
            objects: ids(&mut item)?,
            degrees: match item.need::<f32>("degrees", "positive clockwise")? {
                degrees if degrees.is_finite() => degrees,
                degrees => return Err(format!("`degrees` is {degrees}; give a number")),
            },
            around: item.take::<[f32; 2]>("around")?.map(point).transpose()?,
        },
        "set" => {
            let object = item.need::<u64>("object", "the id Document lists")?;
            let fields: Map<String, Json> = item.need("item", "the object as a Draw item")?;
            let tool = fields
                .get("tool")
                .and_then(Json::as_str)
                .unwrap_or_default();
            if !OBJECT_ITEMS.contains(&tool) {
                return Err(format!(
                    "`item` draws an object; its `tool` is one of {}",
                    listed(OBJECT_ITEMS)
                ));
            }
            ObjectStep::Set {
                object,
                item: drawn(fields)?,
            }
        }
        "delete" => ObjectStep::Delete {
            objects: ids(&mut item)?,
        },
        "arrange" => ObjectStep::Arrange {
            objects: ids(&mut item)?,
            stack: item.named("stack", &STACKINGS)?.ok_or_else(|| {
                format!(
                    "arrange needs `stack`: {}",
                    listed(STACKINGS.map(|(name, _)| name))
                )
            })?,
        },
        "duplicate" => ObjectStep::Duplicate {
            objects: ids(&mut item)?,
            offset: offset(&mut item)?.unwrap_or(Point::new(STEP, STEP)),
        },
        _ => ObjectStep::Rasterize {
            layer: item.layer("layer")?,
        },
    })
}

fn ids(item: &mut Item) -> Parsed<Vec<u64>> {
    let ids: Vec<u64> = item.need("objects", "[ids] as Document lists them")?;
    match ids.len() {
        0 => Err("`objects` is empty; give the ids Document lists".to_owned()),
        count if count > MAX_OBJECTS => Err(format!(
            "{count} objects were given; at most {MAX_OBJECTS} per step"
        )),
        _ => Ok(ids),
    }
}

fn offset(item: &mut Item) -> Parsed<Option<Point>> {
    item.take::<[f32; 2]>("offset")?.map(point).transpose()
}
