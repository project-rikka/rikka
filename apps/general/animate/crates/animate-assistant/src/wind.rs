use std::collections::BTreeMap;

use animate_wind::WindSetting;
use serde::Deserialize;
use serde_json::Value as Json;

use crate::argument_error::ArgumentError;
use crate::parse::{Parsed, curve, read, unknown, within};
use crate::request::{ToolRequest, WindEdit, WindKeyRequest};
use crate::vocabulary::wind_settings;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WindArgs {
    #[serde(default)]
    frame: Option<u32>,
}

pub(crate) fn wind(input: Json) -> Parsed<ToolRequest> {
    let args: WindArgs = read(input)?;
    Ok(ToolRequest::Wind { frame: args.frame })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetWindArgs {
    #[serde(default)]
    resting: BTreeMap<String, f32>,
    #[serde(default)]
    keys: Vec<WindKeyArgs>,
    #[serde(default)]
    clear: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WindKeyArgs {
    frame: u32,
    values: BTreeMap<String, f32>,
    #[serde(default)]
    curve: Option<Json>,
}

pub(crate) fn set_wind(input: Json) -> Parsed<ToolRequest> {
    let args: SetWindArgs = read(input)?;
    if args.resting.is_empty() && args.keys.is_empty() && args.clear.is_empty() {
        return Err(ArgumentError::Empty {
            expected: "`resting` values, `keys` or settings to `clear`",
        });
    }
    let keys = args
        .keys
        .into_iter()
        .map(|key| {
            let at = ArgumentError::at_frame(key.frame);
            if key.values.is_empty() {
                return Err(at(ArgumentError::Empty {
                    expected: "setting values for the key",
                }));
            }
            Ok(WindKeyRequest {
                frame: key.frame,
                values: values(&key.values).map_err(&at)?,
                curve: key.curve.as_ref().map(curve).transpose().map_err(&at)?,
            })
        })
        .collect::<Parsed<Vec<WindKeyRequest>>>()?;
    Ok(ToolRequest::SetWind(WindEdit {
        resting: values(&args.resting)?,
        keys,
        clear: args
            .clear
            .iter()
            .map(|name| setting(name))
            .collect::<Parsed<Vec<WindSetting>>>()?,
    }))
}

fn setting(name: &str) -> Parsed<WindSetting> {
    WindSetting::named(name).ok_or_else(|| {
        unknown(
            "wind setting",
            name,
            format!("the settings are {}", wind_settings()),
        )
    })
}

fn values(values_by_setting: &BTreeMap<String, f32>) -> Parsed<Vec<(WindSetting, f32)>> {
    values_by_setting
        .iter()
        .map(|(name, value)| {
            let setting = setting(name)?;
            let range = setting.range();
            Ok((setting, within(*value, *range.start(), *range.end(), name)?))
        })
        .collect()
}
