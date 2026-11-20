mod adjust;
mod draw;
mod file;
mod item;
mod layers;
mod objects;
mod select;
mod transform;

use paint_document::Backdrop;
use rikka_types::Color;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value as Json;

use crate::framing::{LONGEST, MAX_SIDE};
use crate::ops::Parsed;
use crate::order::{
    Area, Grid, HistoryOrder, MeasureOrder, NewOrder, Order, Rect, Request, ViewOrder,
};
use crate::vocabulary::{HISTORY, color};

pub(crate) use adjust::adjust;
pub(crate) use draw::draw;
pub(crate) use file::{open, save};
use item::layer_ref;
pub(crate) use layers::layers;
pub(crate) use objects::objects;
pub(crate) use select::select;
pub(crate) use transform::transform;

const MAX_POINTS: usize = 64;
const MAX_RADIUS: u32 = 50;
const MIN_LONGEST: u32 = 64;

pub(crate) fn read<T: DeserializeOwned>(input: Json) -> Parsed<T> {
    serde_json::from_value(input).map_err(|error| format!("invalid arguments: {error}"))
}

pub(crate) fn rect(coords: [i64; 4], what: &str) -> Parsed<Rect> {
    let [left, top, width, height] = coords;
    if left < 0 || top < 0 {
        return Err(format!(
            "{what} starts at ({left}, {top}); document pixels start at (0, 0)"
        ));
    }
    if width < 1 || height < 1 {
        return Err(format!(
            "{what} is {width}×{height}; give a width and height of at least 1"
        ));
    }
    let fit = |value: i64| u32::try_from(value).map_err(|_| format!("{what} is too large"));
    Ok(Rect {
        left: fit(left)?,
        top: fit(top)?,
        width: fit(width)?,
        height: fit(height)?,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentArgs {
    #[serde(default)]
    document: Option<String>,
}

pub(crate) fn document(input: Json) -> Parsed<Order> {
    let args: DocumentArgs = read(input)?;
    Ok(Order {
        document: args.document,
        request: Request::Document,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewArgs {
    #[serde(default)]
    document: Option<String>,
    #[serde(default)]
    region: Option<[i64; 4]>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    layer: Option<Json>,
    #[serde(default)]
    grid: Option<Json>,
    #[serde(default)]
    selection: Option<bool>,
    #[serde(default)]
    compare: bool,
}

pub(crate) fn view(input: Json) -> Parsed<Order> {
    let args: ViewArgs = read(input)?;
    let longest = args.width.unwrap_or(LONGEST);
    if !(MIN_LONGEST..=MAX_SIDE).contains(&longest) {
        return Err(format!(
            "width is {longest}; ask for {MIN_LONGEST}–{MAX_SIDE} px along the longer side"
        ));
    }
    let grid = match args.grid {
        None | Some(Json::Bool(false)) => None,
        Some(Json::Bool(true)) => Some(Grid::Auto),
        Some(Json::Number(step)) => match step.as_u64().and_then(|step| u32::try_from(step).ok()) {
            Some(step) if step > 0 => Some(Grid::Every(step)),
            _ => {
                return Err(format!(
                    "grid is {step}; give a step of whole document pixels"
                ));
            }
        },
        Some(other) => {
            return Err(format!(
                "grid is {other}; use true for a chosen step or a step in document pixels"
            ));
        }
    };
    Ok(Order {
        document: args.document,
        request: Request::View(ViewOrder {
            region: args
                .region
                .map(|region| rect(region, "the region"))
                .transpose()?,
            longest,
            layer: args.layer.as_ref().map(layer_ref).transpose()?,
            grid,
            selection: args.selection,
            compare: args.compare,
        }),
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MeasureArgs {
    #[serde(default)]
    document: Option<String>,
    #[serde(default)]
    area: Option<Json>,
    #[serde(default)]
    layer: Option<Json>,
    #[serde(default)]
    points: Vec<[i64; 2]>,
    #[serde(default)]
    radius: Option<u32>,
}

pub(crate) fn measure(input: Json) -> Parsed<Order> {
    let args: MeasureArgs = read(input)?;
    let area = match args.area {
        None => None,
        Some(Json::String(named)) => Some(match named.as_str() {
            "selection" => Area::Selection,
            "whole" => Area::Whole,
            _ => {
                return Err(format!(
                    "area is \"{named}\"; use \"selection\", \"whole\" or [x, y, width, height]"
                ));
            }
        }),
        Some(value) => {
            let region: [i64; 4] = serde_json::from_value(value.clone()).map_err(|_| {
                format!("area is {value}; use \"selection\", \"whole\" or [x, y, width, height]")
            })?;
            Some(Area::Region(rect(region, "the area")?))
        }
    };
    if args.points.len() > MAX_POINTS {
        return Err(format!(
            "{} points were asked; sample at most {MAX_POINTS} at once",
            args.points.len()
        ));
    }
    let points = args
        .points
        .iter()
        .map(|[x, y]| match (u32::try_from(*x), u32::try_from(*y)) {
            (Ok(x), Ok(y)) => Ok((x, y)),
            _ => Err(format!("({x}, {y}) lies outside the document")),
        })
        .collect::<Parsed<Vec<(u32, u32)>>>()?;
    let radius = args.radius.unwrap_or(0);
    if radius > MAX_RADIUS {
        return Err(format!(
            "radius is {radius}; average at most {MAX_RADIUS} px around a point"
        ));
    }
    let area = match (area, points.is_empty()) {
        (None, true) => Some(Area::Chosen),
        (area, _) => area,
    };
    Ok(Order {
        document: args.document,
        request: Request::Measure(MeasureOrder {
            area,
            layer: args.layer.as_ref().map(layer_ref).transpose()?,
            points,
            radius,
        }),
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryArgs {
    #[serde(default)]
    document: Option<String>,
    action: String,
    #[serde(default)]
    steps: Option<u32>,
    #[serde(default)]
    index: Option<usize>,
}

pub(crate) fn history(input: Json) -> Parsed<Order> {
    let args: HistoryArgs = read(input)?;
    let steps = args.steps.unwrap_or(1).max(1);
    let travel = match args.action.as_str() {
        "list" => HistoryOrder::List,
        "undo" => HistoryOrder::Undo(steps),
        "redo" => HistoryOrder::Redo(steps),
        "revisit" => HistoryOrder::Revisit(
            args.index
                .ok_or("revisit needs the `index` of a history entry, as History list shows")?,
        ),
        other => {
            return Err(format!(
                "there is no action \"{other}\"; use {}",
                HISTORY.join(", ")
            ));
        }
    };
    Ok(Order {
        document: args.document,
        request: Request::History(travel),
    })
}

const MAX_NEW: u32 = 30_000;
const RESOLUTIONS: std::ops::RangeInclusive<f64> = 1.0..=10_000.0;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewArgs {
    width: u32,
    height: u32,
    #[serde(default)]
    resolution: Option<f64>,
    #[serde(default)]
    background: Option<Json>,
}

pub(crate) fn new(input: Json) -> Parsed<Order> {
    let args: NewArgs = read(input)?;
    for (side, value) in [("width", args.width), ("height", args.height)] {
        if !(1..=MAX_NEW).contains(&value) {
            return Err(format!("{side} is {value}; use 1–{MAX_NEW} px"));
        }
    }
    let resolution = args.resolution.unwrap_or(72.0);
    if !RESOLUTIONS.contains(&resolution) {
        return Err(format!(
            "resolution is {resolution}; use 1–10000 pixels per inch"
        ));
    }
    let backdrop = match args.background {
        None => Backdrop::Color(Color::WHITE),
        Some(Json::String(word)) if word == "transparent" => Backdrop::Transparent,
        Some(value) => Backdrop::Color(
            color(&value).map_err(|error| format!("background: {error}, or \"transparent\""))?,
        ),
    };
    Ok(Order {
        document: None,
        request: Request::New(NewOrder {
            width: args.width,
            height: args.height,
            resolution,
            backdrop,
        }),
    })
}
