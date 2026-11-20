use std::path::Path;

use rikka_image_edit::{Intent, Profile, Resampling};
use serde::Deserialize;
use serde_json::Value as Json;

use super::item::{changed, counted};
use super::{read, rect};
use crate::ops::Parsed;
use crate::order::{Change, Cropping, Order, TransformStep};
use crate::vocabulary::{
    ANCHORS, FLIPS, INTENTS, PROFILES, RESAMPLINGS, TRANSFORMS, TURNS, color, named, names,
};

const MAX_ICC_BYTES: u64 = 16 << 20;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransformArgs {
    #[serde(default)]
    document: Option<String>,
    name: String,
    steps: Vec<TransformStepArgs>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransformStepArgs {
    op: String,
    #[serde(default)]
    rect: Option<[i64; 4]>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    resampling: Option<String>,
    #[serde(default)]
    anchor: Option<String>,
    #[serde(default)]
    fill: Option<Json>,
    #[serde(default)]
    degrees: Option<f32>,
    #[serde(default)]
    direction: Option<String>,
    #[serde(default)]
    axis: Option<String>,
    #[serde(default)]
    profile: Option<String>,
    #[serde(default)]
    intent: Option<String>,
}

pub(crate) fn transform(input: Json) -> Parsed<Order> {
    let args: TransformArgs = read(input)?;
    counted(&args.steps, "steps")?;
    let transformed = args
        .steps
        .into_iter()
        .enumerate()
        .map(|(at, step)| {
            let op = step.op.clone();
            transform_step(step).map_err(|error| format!("step {} ({op}): {error}", at + 1))
        })
        .collect::<Parsed<Vec<TransformStep>>>()?;
    changed(args.document, args.name, Change::Transform(transformed))
}

fn transform_step(step: TransformStepArgs) -> Parsed<TransformStep> {
    let provided = [
        ("rect", step.rect.is_some()),
        ("width", step.width.is_some()),
        ("height", step.height.is_some()),
        ("resampling", step.resampling.is_some()),
        ("anchor", step.anchor.is_some()),
        ("fill", step.fill.is_some()),
        ("degrees", step.degrees.is_some()),
        ("direction", step.direction.is_some()),
        ("axis", step.axis.is_some()),
        ("profile", step.profile.is_some()),
        ("intent", step.intent.is_some()),
    ];
    let takes: &[&str] = match step.op.as_str() {
        "crop" => &["rect"],
        "resize" => &["width", "height", "resampling"],
        "canvas_size" => &["width", "height", "anchor", "fill"],
        "rotate" => &["degrees", "resampling", "fill"],
        "turn" => &["direction"],
        "flip" => &["axis"],
        "convert" => &["profile", "intent"],
        other => {
            return Err(format!(
                "there is no transform \"{other}\"; use {}",
                TRANSFORMS.join(", ")
            ));
        }
    };
    if let Some((stray, _)) = provided
        .iter()
        .find(|(field, present)| *present && !takes.contains(field))
    {
        return Err(format!(
            "`{stray}` is not for {}; it takes {}",
            step.op,
            takes.join(", ")
        ));
    }
    let resampling = |requested: Option<&str>, fallback: Resampling| {
        requested
            .map(|name| named(&RESAMPLINGS, name, "resampling"))
            .transpose()
            .map(|resampling| resampling.unwrap_or(fallback))
    };
    let fill = step.fill.as_ref().map(color).transpose()?;
    Ok(match step.op.as_str() {
        "crop" => TransformStep::Crop(match step.rect {
            Some(area) => Cropping::Rect(rect(area, "rect")?),
            None => Cropping::Selection,
        }),
        "resize" => {
            if step.width.is_none() && step.height.is_none() {
                return Err(
                    "give `width`, `height` or both; one alone keeps the proportions".to_owned(),
                );
            }
            TransformStep::Resize {
                width: positive(step.width, "width")?,
                height: positive(step.height, "height")?,
                resampling: resampling(step.resampling.as_deref(), Resampling::default())?,
            }
        }
        "canvas_size" => {
            if step.width.is_none() && step.height.is_none() {
                return Err("give `width`, `height` or both".to_owned());
            }
            TransformStep::CanvasSize {
                width: positive(step.width, "width")?,
                height: positive(step.height, "height")?,
                anchor: step
                    .anchor
                    .as_deref()
                    .map(|anchor| named(&ANCHORS, anchor, "anchor"))
                    .transpose()?
                    .unwrap_or_default(),
                fill,
            }
        }
        "rotate" => {
            let degrees = step
                .degrees
                .ok_or("give `degrees`; positive turns clockwise")?;
            if !degrees.is_finite() || degrees.abs() > 360.0 {
                return Err(format!("degrees is {degrees}; use −360 to 360"));
            }
            TransformStep::Rotate {
                degrees,
                resampling: resampling(step.resampling.as_deref(), Resampling::Bicubic)?,
                fill,
            }
        }
        "turn" => TransformStep::Turn(named(
            &TURNS,
            step.direction.as_deref().unwrap_or_default(),
            "direction",
        )?),
        "flip" => TransformStep::Flip(named(
            &FLIPS,
            step.axis.as_deref().unwrap_or_default(),
            "axis",
        )?),
        _ => TransformStep::Convert {
            profile: profile(step.profile.as_deref().unwrap_or_default())?,
            intent: step
                .intent
                .as_deref()
                .map(|intent| named(&INTENTS, intent, "intent"))
                .transpose()?
                .unwrap_or(Intent::RelativeColorimetric),
        },
    })
}

fn positive(value: Option<u32>, what: &str) -> Parsed<Option<u32>> {
    match value {
        Some(0) => Err(format!("{what} is 0; give at least 1 px")),
        other => Ok(other),
    }
}

fn profile(profile_name: &str) -> Parsed<Profile> {
    if let Some((_, made)) = PROFILES.iter().find(|(name, _)| *name == profile_name) {
        return Ok(made());
    }
    let path = Path::new(profile_name);
    if !path.is_absolute() {
        return Err(format!(
            "there is no profile \"{profile_name}\"; use {} or the absolute path of an .icc file",
            names(&PROFILES).join(", ")
        ));
    }
    let size = std::fs::metadata(path)
        .map_err(|error| format!("{profile_name} cannot be read: {error}"))?
        .len();
    if size > MAX_ICC_BYTES {
        return Err(format!("{profile_name} is too large for a colour profile"));
    }
    let bytes =
        std::fs::read(path).map_err(|error| format!("{profile_name} cannot be read: {error}"))?;
    Profile::from_icc(bytes)
        .map_err(|error| format!("{profile_name} is no colour profile: {error}"))
}
