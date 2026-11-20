use std::collections::BTreeMap;

use animate_cloth::{Knob, preset_named};
use serde::Deserialize;
use serde_json::Value as Json;

use crate::argument_error::ArgumentError;
use crate::parse::{Parsed, read, unknown, within};
use crate::request::{PieceRequest, ToolRequest};
use crate::vocabulary::{knobs, presets};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClothArgs {
    figure: String,
}

pub(crate) fn cloth(input: Json) -> Parsed<ToolRequest> {
    let args: ClothArgs = read(input)?;
    Ok(ToolRequest::Cloth {
        figure: args.figure,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetClothArgs {
    figure: String,
    #[serde(default)]
    feel: BTreeMap<String, f32>,
    #[serde(default)]
    pieces: Vec<PieceArgs>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PieceArgs {
    piece: String,
    #[serde(default)]
    preset: Option<String>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    feel: BTreeMap<String, f32>,
    #[serde(default)]
    reset: bool,
}

pub(crate) fn set_cloth(input: Json) -> Parsed<ToolRequest> {
    let args: SetClothArgs = read(input)?;
    let feel = feels(&args.feel)?;
    if feel.is_empty() && args.pieces.is_empty() {
        return Err(ArgumentError::Empty {
            expected: "the whole figure's feel or at least one piece",
        });
    }
    let pieces = args
        .pieces
        .into_iter()
        .map(piece)
        .collect::<Parsed<Vec<PieceRequest>>>()?;
    Ok(ToolRequest::SetCloth {
        figure: args.figure,
        feel,
        pieces,
    })
}

fn feels(feels_by_knob: &BTreeMap<String, f32>) -> Parsed<Vec<(Knob, f32)>> {
    feels_by_knob
        .iter()
        .map(|(name, value)| {
            let knob = Knob::named(name)
                .ok_or_else(|| unknown("feel", name, format!("the feels are {}", knobs())))?;
            Ok((knob, within(*value, 0.0, 1.0, name)?))
        })
        .collect()
}

fn piece(args: PieceArgs) -> Parsed<PieceRequest> {
    let preset = args
        .preset
        .map(|name| {
            preset_named(&name)
                .ok_or_else(|| unknown("preset", &name, format!("the presets are {}", presets())))
        })
        .transpose()?;
    let feel = feels(&args.feel)?;
    if preset.is_none() && args.enabled.is_none() && feel.is_empty() && !args.reset {
        return Err(ArgumentError::EmptyPiece { piece: args.piece });
    }
    Ok(PieceRequest {
        piece: args.piece,
        preset,
        enabled: args.enabled,
        feel,
        reset: args.reset,
    })
}
