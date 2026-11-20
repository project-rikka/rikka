use std::ops::RangeInclusive;

use libspa::param::ParamType;
use libspa::param::audio::AudioFormat;
use libspa::pod::{ChoiceValue, Pod, Property, Value};
use libspa::sys;
use libspa::utils::{Choice, ChoiceEnum, Id, SpaTypes};
use rikka_pipewire::{Fields, OfferedRates};

use crate::config::{ALLOWED_RATES, DeviceTuning, FORMAT, RATE, Setting};
use crate::format::SampleFormat;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Offer {
    pub(super) formats: Vec<SampleFormat>,
    pub(super) rates: Option<RangeInclusive<u32>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Pins {
    pub(super) format: bool,
    pub(super) rate: bool,
}

pub(super) fn offer(param: &Pod) -> Option<Offer> {
    let fields = Fields::parse(param)?;
    if fields.id(sys::SPA_FORMAT_mediaSubtype)? != sys::SPA_MEDIA_SUBTYPE_raw {
        return None;
    }
    let formats = fields
        .get(sys::SPA_FORMAT_AUDIO_format)
        .map(ids)
        .unwrap_or_default()
        .into_iter()
        .filter_map(pinnable)
        .collect();
    let rates = fields
        .get(sys::SPA_FORMAT_AUDIO_rate)
        .and_then(OfferedRates::parse)
        .as_ref()
        .and_then(OfferedRates::span);
    Some(Offer { formats, rates })
}

pub(super) fn pins(param: &Pod) -> Option<Pins> {
    let fields = Fields::parse(param)?;
    let Value::Struct(values) = fields.get(sys::SPA_PROP_params)? else {
        return None;
    };
    let mut format = None;
    let mut rate = false;
    let mut allowed = false;
    for pair in values.as_chunks::<2>().0 {
        let [Value::String(key), value] = pair else {
            continue;
        };
        match (key.as_str(), value) {
            (FORMAT, Value::String(name)) => {
                format = Some(!name.is_empty() && name != "UNKNOWN");
            }
            (RATE, Value::Int(pinned)) => rate = *pinned != 0,
            (ALLOWED_RATES, Value::String(rates)) => {
                allowed = rates.contains(|c: char| c.is_ascii_digit());
            }
            _ => {}
        }
    }
    Some(Pins {
        format: format?,
        rate: rate || allowed,
    })
}

pub(super) fn tuning_param(tuning: &DeviceTuning) -> Vec<u8> {
    let params = tuning
        .settings()
        .into_iter()
        .flat_map(|(key, setting)| {
            let value = match setting {
                Setting::Text(text) => Value::String(text),
                Setting::Number(number) => Value::Int(i32::try_from(number).unwrap_or(i32::MAX)),
                Setting::Switch(on) => Value::Bool(on),
            };
            [Value::String(key.to_owned()), value]
        })
        .collect();
    rikka_pipewire::object(
        SpaTypes::ObjectParamProps,
        ParamType::Props,
        vec![Property::new(sys::SPA_PROP_params, Value::Struct(params))],
    )
}

fn ids(value: &Value) -> Vec<u32> {
    match value {
        Value::Id(Id(id)) => vec![*id],
        Value::Choice(ChoiceValue::Id(Choice(_, ChoiceEnum::None(Id(id))))) => vec![*id],
        Value::Choice(ChoiceValue::Id(Choice(
            _,
            ChoiceEnum::Enum {
                default,
                alternatives,
            },
        ))) => std::iter::once(default)
            .chain(alternatives)
            .map(|Id(id)| *id)
            .collect(),
        _ => Vec::new(),
    }
}

fn pinnable(id: u32) -> Option<SampleFormat> {
    match AudioFormat(id) {
        AudioFormat::S16LE => Some(SampleFormat::S16),
        AudioFormat::S24LE => Some(SampleFormat::S24),
        AudioFormat::S24_32LE => Some(SampleFormat::S24In32),
        AudioFormat::S32LE => Some(SampleFormat::S32),
        AudioFormat::F32LE => Some(SampleFormat::F32),
        AudioFormat::F64LE => Some(SampleFormat::F64),
        _ => None,
    }
}
