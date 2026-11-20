use libspa::param::ParamType;
use libspa::pod::{Pod, Property, Value, ValueArray};
use libspa::sys;
use libspa::utils::SpaTypes;
use rikka_pipewire::{Fields, RawFormat, Sample, object};

use crate::device::{Profile, Route};
use crate::format::{Format, SampleFormat};
use crate::volume::Volume;

pub(crate) fn volume(param: &Pod) -> Option<Volume> {
    let fields = Fields::parse(param)?;
    let channels = fields.floats(sys::SPA_PROP_channelVolumes)?.to_vec();
    let muted = fields.bool(sys::SPA_PROP_mute).unwrap_or(false);
    Some(Volume::new(channels, muted))
}

pub(crate) fn volume_param(volume: &Volume) -> Vec<u8> {
    object(
        SpaTypes::ObjectParamProps,
        ParamType::Props,
        vec![
            Property::new(sys::SPA_PROP_mute, Value::Bool(volume.muted())),
            Property::new(
                sys::SPA_PROP_channelVolumes,
                Value::ValueArray(ValueArray::Float(volume.channels().to_vec())),
            ),
        ],
    )
}

pub(crate) fn latency_offset_param(nanoseconds: i64) -> Vec<u8> {
    object(
        SpaTypes::ObjectParamProps,
        ParamType::Props,
        vec![Property::new(
            sys::SPA_PROP_latencyOffsetNsec,
            Value::Long(nanoseconds),
        )],
    )
}

pub(crate) fn latency_offset(param: &Pod) -> Option<i64> {
    Fields::parse(param)?.long(sys::SPA_PROP_latencyOffsetNsec)
}

pub(crate) fn profile(param: &Pod) -> Option<Profile> {
    let fields = Fields::parse(param)?;
    let index = fields.int(sys::SPA_PARAM_PROFILE_index)?;
    let name = fields
        .string(sys::SPA_PARAM_PROFILE_name)
        .unwrap_or_default()
        .to_owned();
    Some(Profile {
        index,
        description: fields
            .string(sys::SPA_PARAM_PROFILE_description)
            .map_or_else(|| name.clone(), str::to_owned),
        name,
        available: fields
            .id(sys::SPA_PARAM_PROFILE_available)
            .is_none_or(|availability| availability != sys::SPA_PARAM_AVAILABILITY_no),
    })
}

pub(crate) fn profile_param(index: i32) -> Vec<u8> {
    object(
        SpaTypes::ObjectParamProfile,
        ParamType::Profile,
        vec![
            Property::new(sys::SPA_PARAM_PROFILE_index, Value::Int(index)),
            Property::new(sys::SPA_PARAM_PROFILE_save, Value::Bool(true)),
        ],
    )
}

pub(crate) fn route(param: &Pod) -> Option<Route> {
    let fields = Fields::parse(param)?;
    let index = fields.int(sys::SPA_PARAM_ROUTE_index)?;
    let name = fields
        .string(sys::SPA_PARAM_ROUTE_name)
        .unwrap_or_default()
        .to_owned();
    let device = fields
        .int(sys::SPA_PARAM_ROUTE_device)
        .or_else(|| fields.ints(sys::SPA_PARAM_ROUTE_devices)?.first().copied())?;
    Some(Route {
        index,
        device,
        description: fields
            .string(sys::SPA_PARAM_ROUTE_description)
            .map_or_else(|| name.clone(), str::to_owned),
        name,
        output: fields.id(sys::SPA_PARAM_ROUTE_direction) == Some(sys::SPA_DIRECTION_OUTPUT),
        available: match fields.id(sys::SPA_PARAM_ROUTE_available) {
            Some(sys::SPA_PARAM_AVAILABILITY_yes) => Some(true),
            Some(sys::SPA_PARAM_AVAILABILITY_no) => Some(false),
            _ => None,
        },
        active: false,
    })
}

pub(crate) fn route_param(index: i32, device: i32) -> Vec<u8> {
    object(
        SpaTypes::ObjectParamRoute,
        ParamType::Route,
        vec![
            Property::new(sys::SPA_PARAM_ROUTE_index, Value::Int(index)),
            Property::new(sys::SPA_PARAM_ROUTE_device, Value::Int(device)),
            Property::new(sys::SPA_PARAM_ROUTE_save, Value::Bool(true)),
        ],
    )
}

pub(crate) fn format(param: &Pod) -> Option<Format> {
    let raw = RawFormat::parse(param)?;
    Some(Format {
        sample: sample_format(raw.sample?),
        rate: raw.rate,
        channels: raw.channels,
    })
}

fn sample_format(sample: Sample) -> SampleFormat {
    match sample {
        Sample::S16 => SampleFormat::S16,
        Sample::S24 => SampleFormat::S24,
        Sample::S24In32 => SampleFormat::S24In32,
        Sample::S32 => SampleFormat::S32,
        Sample::F32 => SampleFormat::F32,
        Sample::F64 => SampleFormat::F64,
    }
}
