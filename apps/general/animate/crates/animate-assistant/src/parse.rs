use std::collections::BTreeMap;

use animate_body::{Expression, HandPreset, Limb, Side, muscle_of, muscles};
use animate_motion::{GazeMode, Part, Tag};
use animate_sheet::Lane;
use animate_solve::EyeDirection;
use rikka_mmd::VmdCurve;
use rikka_model_core::glam::Vec3;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value as Json;

use crate::argument_error::ArgumentError;
use crate::look::look_lane;
use crate::request::{
    CameraKey, CameraPosition, Extremity, GazeRequest, Hips, JointRotation, KeysAction,
    KeysRequest, KeysSubject, Placement, PoseKey, RenderRequest, RenderView, ToolRequest,
};
use crate::vocabulary::{
    curve_named, curves, dof_name, dof_named, is_posable, joint_named, side_name,
};

pub(crate) type Parsed<T> = Result<T, ArgumentError>;

const FOV: f32 = 30.0;
const WIDTH: u32 = 640;
const NARROWEST: u32 = 64;

const LIST: &str = "list";
const DELETE: &str = "delete";
const SHIFT: &str = "shift";
const CURVE: &str = "curve";
const TAG: &str = "tag";
pub(crate) const ACTIONS: [&str; 5] = [LIST, DELETE, SHIFT, CURVE, TAG];

const FILM: &str = "film";
const VIEW: &str = "view";
pub(crate) const FILMS: [&str; 2] = [FILM, VIEW];

pub(crate) fn read<T: DeserializeOwned>(input: Json) -> Parsed<T> {
    serde_json::from_value(input).map_err(ArgumentError::Malformed)
}

fn finite(value: f32, what: &str) -> Parsed<f32> {
    match value.is_finite() {
        true => Ok(value),
        false => Err(ArgumentError::NotFinite {
            what: what.to_owned(),
        }),
    }
}

fn point(value: [f32; 3], what: &str) -> Parsed<Vec3> {
    for coordinate in value {
        finite(coordinate, what)?;
    }
    Ok(Vec3::from_array(value))
}

pub(crate) fn within(value: f32, min: f32, max: f32, what: &str) -> Parsed<f32> {
    within_unit(value, min, max, "", what)
}

fn within_unit(value: f32, min: f32, max: f32, unit: &'static str, what: &str) -> Parsed<f32> {
    match value >= min && value <= max {
        true => Ok(value),
        false => Err(ArgumentError::OutOfRange {
            what: what.to_owned(),
            value,
            min,
            max,
            unit,
        }),
    }
}

fn weight(value: f32, what: &str) -> Parsed<f32> {
    within(value, 0.0, 1.0, what)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoArgs {}

pub(crate) fn scene(input: Json) -> Parsed<ToolRequest> {
    let _: NoArgs = read(input)?;
    Ok(ToolRequest::Scene)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FigureArgs {
    figure: String,
    #[serde(default)]
    frame: Option<u32>,
}

pub(crate) fn figure(input: Json) -> Parsed<ToolRequest> {
    let args: FigureArgs = read(input)?;
    Ok(ToolRequest::Figure {
        figure: args.figure,
        frame: args.frame,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PoseArgs {
    figure: String,
    keys: Vec<PoseKeyArgs>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PoseKeyArgs {
    frame: u32,
    #[serde(default)]
    tag: Option<String>,
    #[serde(default)]
    rest: Vec<String>,
    #[serde(default)]
    joints: BTreeMap<String, BTreeMap<String, f32>>,
    #[serde(default)]
    hips: Option<HipsArgs>,
    #[serde(default)]
    hands: BTreeMap<String, String>,
    #[serde(default)]
    expressions: BTreeMap<String, f32>,
    #[serde(default)]
    morphs: BTreeMap<String, f32>,
    #[serde(default)]
    gaze: Option<GazeArgs>,
    #[serde(default)]
    reach: BTreeMap<String, [f32; 3]>,
    #[serde(default)]
    pins: BTreeMap<String, bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HipsArgs {
    #[serde(default)]
    position: Option<[f32; 3]>,
    #[serde(default)]
    yaw: Option<f32>,
    #[serde(default)]
    pitch: Option<f32>,
    #[serde(default)]
    roll: Option<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct GazeArgs {
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    target: Option<[f32; 3]>,
    #[serde(default)]
    weight: Option<f32>,
    #[serde(default)]
    head_follow: Option<f32>,
    #[serde(default)]
    eyes: BTreeMap<String, [f32; 2]>,
}

pub(crate) fn pose(input: Json) -> Parsed<ToolRequest> {
    let args: PoseArgs = read(input)?;
    if args.keys.is_empty() {
        return Err(ArgumentError::Empty {
            expected: "at least one key",
        });
    }
    let keys = args
        .keys
        .into_iter()
        .map(pose_key)
        .collect::<Parsed<Vec<PoseKey>>>()?;
    Ok(ToolRequest::Pose {
        figure: args.figure,
        keys,
    })
}

fn pose_key(args: PoseKeyArgs) -> Parsed<PoseKey> {
    let at = ArgumentError::at_frame(args.frame);
    Ok(PoseKey {
        frame: args.frame,
        tag: args.tag.as_deref().map(tag).transpose().map_err(&at)?,
        rest: rest(&args.rest).map_err(&at)?,
        joints: joints(&args.joints).map_err(&at)?,
        hips: args.hips.map(hips).transpose().map_err(&at)?,
        hands: hands(&args.hands).map_err(&at)?,
        expressions: expressions(&args.expressions).map_err(&at)?,
        morphs: args
            .morphs
            .iter()
            .map(|(name, value)| Ok((name.clone(), weight(*value, name)?)))
            .collect::<Parsed<Vec<(String, f32)>>>()
            .map_err(&at)?,
        gaze: args.gaze.map(gaze).transpose().map_err(&at)?,
        reach: reach(&args.reach).map_err(&at)?,
        pins: pins(&args.pins).map_err(&at)?,
    })
}

pub(crate) fn unknown(kind: &'static str, name: &str, expected: String) -> ArgumentError {
    ArgumentError::Unknown {
        kind,
        name: name.to_owned(),
        expected,
    }
}

fn tag(name: &str) -> Parsed<Tag> {
    Tag::named(name).ok_or_else(|| {
        unknown(
            "tag",
            name,
            format!("the tags are {}", Tag::ALL.map(Tag::name).join(", ")),
        )
    })
}

fn part(name: &str) -> Parsed<Part> {
    Part::named(name).ok_or_else(|| {
        unknown(
            "part",
            name,
            format!("the parts are {}", Part::ALL.map(Part::name).join(", ")),
        )
    })
}

fn rest(names: &[String]) -> Parsed<Vec<Part>> {
    if names.iter().any(|name| name == "all") {
        return Ok(Part::ALL.to_vec());
    }
    names.iter().map(|name| part(name)).collect()
}

fn joints(angles_by_joint: &BTreeMap<String, BTreeMap<String, f32>>) -> Parsed<Vec<JointRotation>> {
    let mut turns = Vec::new();
    for (name, angles) in angles_by_joint {
        let bone = joint_named(name).ok_or_else(|| {
            unknown(
                "joint",
                name,
                "the joints are listed in the tool's description".to_owned(),
            )
        })?;
        for (dof, degrees) in angles {
            let dof = dof_named(dof).ok_or_else(|| ArgumentError::UnknownAxis {
                joint: name.clone(),
                dof: dof.clone(),
            })?;
            if muscle_of(bone, dof).is_none_or(|index| !is_posable(index)) {
                let dofs: Vec<&str> = muscles()
                    .iter()
                    .filter(|muscle| muscle.bone == bone)
                    .map(|muscle| dof_name(muscle.dof))
                    .collect();
                return Err(ArgumentError::FixedAxis {
                    joint: name.clone(),
                    allowed: dofs.join(", "),
                });
            }
            turns.push(JointRotation {
                bone,
                dof,
                degrees: finite(*degrees, name)?,
            });
        }
    }
    Ok(turns)
}

fn hips(args: HipsArgs) -> Parsed<Hips> {
    Ok(Hips {
        position: args
            .position
            .map(|position| point(position, "the hips' position"))
            .transpose()?,
        yaw: args.yaw.map(|yaw| finite(yaw, "yaw")).transpose()?,
        pitch: args.pitch.map(|pitch| finite(pitch, "pitch")).transpose()?,
        roll: args.roll.map(|roll| finite(roll, "roll")).transpose()?,
    })
}

fn side(name: &str) -> Parsed<Side> {
    Side::BOTH
        .into_iter()
        .find(|side| side_name(*side) == name)
        .ok_or_else(|| unknown("side", name, "use left or right".to_owned()))
}

fn hands(presets_by_hand: &BTreeMap<String, String>) -> Parsed<Vec<(Side, HandPreset)>> {
    presets_by_hand
        .iter()
        .map(|(which, shape)| {
            let preset = HandPreset::ALL
                .into_iter()
                .find(|preset| preset.name() == shape)
                .ok_or_else(|| {
                    unknown(
                        "hand shape",
                        shape,
                        format!(
                            "the shapes are {}",
                            HandPreset::ALL.map(HandPreset::name).join(", ")
                        ),
                    )
                })?;
            Ok((side(which)?, preset))
        })
        .collect()
}

fn expressions(strengths: &BTreeMap<String, f32>) -> Parsed<Vec<(Expression, f32)>> {
    strengths
        .iter()
        .map(|(name, value)| {
            let expression = Expression::ALL
                .into_iter()
                .find(|expression| expression.name() == name)
                .ok_or_else(|| {
                    unknown(
                        "expression",
                        name,
                        format!(
                            "the expressions are {}; use `morphs` for the model's own",
                            Expression::ALL.map(Expression::name).join(", ")
                        ),
                    )
                })?;
            Ok((expression, weight(*value, name)?))
        })
        .collect()
}

const BOTH_EYES: &str = "both";

fn eyes(
    directions_by_eye: &BTreeMap<String, [f32; 2]>,
) -> Parsed<(Option<EyeDirection>, Option<EyeDirection>)> {
    let eye_direction = |[across, up]: [f32; 2]| -> Parsed<EyeDirection> {
        Ok(EyeDirection {
            across: share(across, "an eye's across")?,
            up: share(up, "an eye's up")?,
        })
    };
    let both = directions_by_eye
        .get(BOTH_EYES)
        .copied()
        .map(eye_direction)
        .transpose()?;
    let (mut left, mut right) = (both, both);
    for (name, direction) in directions_by_eye
        .iter()
        .filter(|(name, _)| *name != BOTH_EYES)
    {
        let eye =
            side(name).map_err(|_| unknown("eye", name, "use both, left or right".to_owned()))?;
        let parsed = Some(eye_direction(*direction)?);
        match eye {
            Side::Left => left = parsed,
            Side::Right => right = parsed,
        }
    }
    Ok((left, right))
}

fn share(value: f32, what: &str) -> Parsed<f32> {
    within(value, -1.0, 1.0, what)
}

fn gaze(args: GazeArgs) -> Parsed<GazeRequest> {
    let mode = args
        .mode
        .as_deref()
        .map(|mode| {
            GazeMode::named(mode).ok_or_else(|| {
                unknown(
                    "gaze mode",
                    mode,
                    format!("use {}", GazeMode::ALL.map(GazeMode::name).join(", ")),
                )
            })
        })
        .transpose()?;
    let (left_eye, right_eye) = eyes(&args.eyes)?;
    Ok(GazeRequest {
        left_eye,
        right_eye,
        mode,
        target: args
            .target
            .map(|target| point(target, "the gaze target"))
            .transpose()?,
        weight: args
            .weight
            .map(|value| weight(value, "the gaze weight"))
            .transpose()?,
        head_follow: args
            .head_follow
            .map(|value| weight(value, "headFollow"))
            .transpose()?,
    })
}

fn reach(targets: &BTreeMap<String, [f32; 3]>) -> Parsed<Vec<(Extremity, Vec3)>> {
    targets
        .iter()
        .map(|(name, target)| {
            let reacher = Extremity::ALL
                .into_iter()
                .find(|reacher| reacher.name() == name)
                .ok_or_else(|| {
                    unknown(
                        "reaching end",
                        name,
                        format!("use {}", Extremity::ALL.map(Extremity::name).join(", ")),
                    )
                })?;
            Ok((reacher, point(*target, name)?))
        })
        .collect()
}

fn pins(pins_by_limb: &BTreeMap<String, bool>) -> Parsed<Vec<(Limb, bool)>> {
    pins_by_limb
        .iter()
        .map(|(name, pinned)| {
            let limb = Part::named(name).and_then(Part::limb).ok_or_else(|| {
                unknown(
                    "pinnable limb",
                    name,
                    format!(
                        "use {}",
                        Limb::ALL.map(|limb| Part::from(limb).name()).join(", ")
                    ),
                )
            })?;
            Ok((limb, *pinned))
        })
        .collect()
}

fn wrong_type(what: &str, expected: impl Into<String>, found: impl ToString) -> ArgumentError {
    ArgumentError::WrongType {
        what: what.to_owned(),
        expected: expected.into(),
        found: found.to_string(),
    }
}
pub(crate) fn curve(json: &Json) -> Parsed<VmdCurve> {
    if let Some(curve_name) = json.as_str() {
        return curve_named(curve_name)
            .ok_or_else(|| unknown("curve", curve_name, format!("the curves are {}", curves())));
    }
    let points: Vec<u8> = json
        .as_array()
        .filter(|points| points.len() == 4)
        .and_then(|points| {
            points
                .iter()
                .map(|point| {
                    point
                        .as_u64()
                        .filter(|point| *point <= u64::from(VmdCurve::SPAN))
                        .and_then(|point| u8::try_from(point).ok())
                })
                .collect()
        })
        .ok_or_else(|| {
            wrong_type(
                "a curve",
                "a name or four whole numbers from 0 to 127",
                json,
            )
        })?;
    Ok(VmdCurve {
        first: (points[0], points[1]),
        second: (points[2], points[3]),
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CameraArgs {
    keys: Vec<serde_json::Map<String, Json>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PlacementArgs {
    target: [f32; 3],
    #[serde(default)]
    eye: Option<[f32; 3]>,
    #[serde(default)]
    yaw: Option<f32>,
    #[serde(default)]
    elevation: Option<f32>,
    #[serde(default)]
    distance: Option<f32>,
    #[serde(default)]
    roll: Option<f32>,
    #[serde(default)]
    fov: Option<f32>,
}

fn placement(args: PlacementArgs) -> Parsed<Placement> {
    let target = point(args.target, "the target")?;
    let orbit = (args.yaw, args.elevation, args.distance);
    let stand = match (args.eye, orbit) {
        (Some(eye), (None, None, None)) => {
            let eye = point(eye, "the eye")?;
            if eye.distance(target) < 0.01 {
                return Err(ArgumentError::EyeOnTarget);
            }
            CameraPosition::Eye(eye)
        }
        (None, (Some(yaw), Some(elevation), Some(distance))) => {
            let distance = finite(distance, "the distance")?;
            if distance < 0.01 {
                return Err(ArgumentError::DistanceOnTarget { distance });
            }
            let elevation = finite(elevation, "the elevation")?;
            if elevation.abs() >= 90.0 {
                return Err(ArgumentError::VerticalElevation { elevation });
            }
            CameraPosition::Orbit {
                yaw: finite(yaw, "the yaw")?,
                elevation,
                distance,
            }
        }
        _ => return Err(ArgumentError::IncompletePlacement),
    };
    let fov = finite(args.fov.unwrap_or(FOV), "the field of view")?;
    within_unit(fov, 1.0, 179.0, "°", "the field of view")?;
    Ok(Placement {
        target,
        stand,
        roll: finite(args.roll.unwrap_or(0.0), "the roll")?,
        fov,
    })
}

pub(crate) fn camera(input: Json) -> Parsed<ToolRequest> {
    let args: CameraArgs = read(input)?;
    if args.keys.is_empty() {
        return Err(ArgumentError::Empty {
            expected: "at least one key",
        });
    }
    let keys = args
        .keys
        .into_iter()
        .map(|mut key| {
            let frame = key
                .remove("frame")
                .and_then(|frame| frame.as_u64())
                .and_then(|frame| u32::try_from(frame).ok())
                .ok_or(ArgumentError::MissingField {
                    action: "camera key",
                    field: "a `frame`",
                })?;
            let at = ArgumentError::at_frame(frame);
            let curve = key
                .remove("curve")
                .as_ref()
                .map(curve)
                .transpose()
                .map_err(&at)?;
            let placed = read::<PlacementArgs>(Json::Object(key)).map_err(&at)?;
            Ok(CameraKey {
                frame,
                placement: placement(placed).map_err(&at)?,
                curve,
            })
        })
        .collect::<Parsed<Vec<CameraKey>>>()?;
    Ok(ToolRequest::Camera { keys })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KeysArgs {
    subject: String,
    action: String,
    #[serde(default)]
    lanes: Vec<Json>,
    #[serde(default)]
    from: Option<u32>,
    #[serde(default)]
    to: Option<u32>,
    #[serde(default)]
    by: Option<i64>,
    #[serde(default)]
    curve: Option<Json>,
    #[serde(default)]
    tag: Option<String>,
}

pub(crate) fn keys(input: Json) -> Parsed<ToolRequest> {
    let args: KeysArgs = read(input)?;
    let subject = match args.subject.as_str() {
        "camera" => KeysSubject::Camera,
        "look" => KeysSubject::Look,
        figure => KeysSubject::Figure(figure.to_owned()),
    };
    let action = match args.action.as_str() {
        LIST => KeysAction::List,
        DELETE => KeysAction::Delete,
        SHIFT => KeysAction::Shift(args.by.filter(|by| *by != 0).ok_or(
            ArgumentError::MissingField {
                action: SHIFT,
                field: "`by`, a number of frames other than 0",
            },
        )?),
        CURVE => match subject {
            KeysSubject::Figure(_) => {
                return Err(ArgumentError::WrongKeyEdit {
                    subject: "figure",
                    expected: "a `tag`",
                    found: "a curve",
                });
            }
            _ => KeysAction::Curve(curve(args.curve.as_ref().ok_or(
                ArgumentError::MissingField {
                    action: CURVE,
                    field: "a `curve`",
                },
            )?)?),
        },
        TAG => match subject {
            KeysSubject::Figure(_) => KeysAction::Tag(tag(args.tag.as_deref().ok_or(
                ArgumentError::MissingField {
                    action: TAG,
                    field: "a `tag`",
                },
            )?)?),
            _ => {
                return Err(ArgumentError::WrongKeyEdit {
                    subject: "camera and look",
                    expected: "a `curve`",
                    found: "a tag",
                });
            }
        },
        other => {
            return Err(unknown(
                "action",
                other,
                format!("use {}", ACTIONS.join(", ")),
            ));
        }
    };
    if let (Some(from), Some(to)) = (args.from, args.to)
        && from > to
    {
        return Err(ArgumentError::EmptyRange { from, to });
    }
    let lanes = args
        .lanes
        .iter()
        .map(|lane| match (&subject, lane) {
            (KeysSubject::Figure(_), Json::String(name)) => part(name).map(Lane::Part),
            (KeysSubject::Look, lane) => look_lane(lane).map(Lane::Look),
            (KeysSubject::Camera, _) => Ok(Lane::Camera),
            (KeysSubject::Figure(_), other) => Err(wrong_type("lane", "a part name", other)),
        })
        .collect::<Parsed<Vec<Lane>>>()?;
    Ok(ToolRequest::Keys(KeysRequest {
        subject,
        action,
        lanes,
        from: args.from,
        to: args.to,
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TimelineArgs {
    #[serde(default)]
    frame: Option<u32>,
    #[serde(default)]
    play: Option<bool>,
}

pub(crate) fn timeline(input: Json) -> Parsed<ToolRequest> {
    let args: TimelineArgs = read(input)?;
    if args.frame.is_none() && args.play.is_none() {
        return Err(ArgumentError::Empty {
            expected: "a `frame` to move to, or `play`",
        });
    }
    Ok(ToolRequest::Timeline {
        frame: args.frame,
        play: args.play,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenderArgs {
    #[serde(default)]
    frames: Vec<u32>,
    #[serde(default)]
    camera: Option<Json>,
    #[serde(default)]
    width: Option<u32>,
}

pub(crate) fn render(input: Json, most_frames: usize, widest: u32) -> Parsed<ToolRequest> {
    let args: RenderArgs = read(input)?;
    if args.frames.len() > most_frames {
        return Err(ArgumentError::TooManyFrames {
            count: args.frames.len(),
            most: most_frames,
        });
    }
    let view = match args.camera {
        None => RenderView::Film,
        Some(Json::String(named)) if named == FILM => RenderView::Film,
        Some(Json::String(named)) if named == VIEW => RenderView::View,
        Some(placed @ Json::Object(_)) => {
            RenderView::Placed(placement(read::<PlacementArgs>(placed)?)?)
        }
        Some(other) => {
            return Err(wrong_type(
                "the camera",
                "`film`, `view` or a placement",
                other,
            ));
        }
    };
    let width = args.width.unwrap_or(WIDTH);
    within(width as f32, NARROWEST as f32, widest as f32, "the width")?;
    Ok(ToolRequest::Render(RenderRequest {
        frames: args.frames,
        view,
        width,
    }))
}
