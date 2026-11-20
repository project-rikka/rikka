use libspa::param::ParamType;
use libspa::param::format::{FormatProperties, MediaSubtype, MediaType};
use libspa::param::video::VideoFormat;
use libspa::pod::deserialize::PodDeserializer;
use libspa::pod::{ChoiceValue, Pod, Property, PropertyFlags, Value};
use libspa::sys;
use libspa::utils::{Choice, ChoiceEnum, ChoiceFlags, Fraction, Id, Rectangle, SpaTypes};
use rikka_gpu::DrmFormat;
use rikka_pipewire::{default_id, default_rectangle, meta, object};

use crate::frames::PixelOrder;

const FORMATS: [VideoFormat; 4] = [
    VideoFormat::BGRx,
    VideoFormat::BGRA,
    VideoFormat::RGBx,
    VideoFormat::RGBA,
];
const LARGEST: u32 = 8192;
const BUFFERS: i32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NegotiatedFormat {
    pub(crate) format: VideoFormat,
    pub(crate) order: PixelOrder,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) modifier: Option<u64>,
}

impl NegotiatedFormat {
    pub(crate) fn fourcc(&self) -> u32 {
        fourcc(self.format)
    }
}

fn fourcc(format: VideoFormat) -> u32 {
    let code = match format {
        VideoFormat::BGRx => *b"XR24",
        VideoFormat::BGRA => *b"AR24",
        VideoFormat::RGBx => *b"XB24",
        _ => *b"AB24",
    };
    u32::from_le_bytes(code)
}

pub(crate) fn order(format: VideoFormat) -> Option<PixelOrder> {
    match format {
        VideoFormat::BGRx | VideoFormat::BGRA => Some(PixelOrder::Bgra),
        VideoFormat::RGBx | VideoFormat::RGBA => Some(PixelOrder::Rgba),
        _ => None,
    }
}

pub(crate) fn offers(
    frame_rate: u32,
    size: Option<(u32, u32)>,
    dmabufs: &[DrmFormat],
) -> Vec<Vec<u8>> {
    let shared = FORMATS.iter().filter_map(|format| {
        let modifiers: Vec<i64> = dmabufs
            .iter()
            .filter(|offered| offered.fourcc == fourcc(*format))
            .map(|offered| offered.modifier as i64)
            .collect();
        (!modifiers.is_empty()).then(|| {
            let mut properties = properties(&[*format], frame_rate, size);
            properties.push(Property {
                key: FormatProperties::VideoModifier.as_raw(),
                flags: PropertyFlags::MANDATORY | PropertyFlags::DONT_FIXATE,
                value: Value::Choice(ChoiceValue::Long(Choice(
                    ChoiceFlags::empty(),
                    ChoiceEnum::Enum {
                        default: modifiers[0],
                        alternatives: modifiers,
                    },
                ))),
            });
            object(
                SpaTypes::ObjectParamFormat,
                ParamType::EnumFormat,
                properties,
            )
        })
    });
    let copied = object(
        SpaTypes::ObjectParamFormat,
        ParamType::EnumFormat,
        properties(&FORMATS, frame_rate, size),
    );
    shared.chain(std::iter::once(copied)).collect()
}

fn properties(formats: &[VideoFormat], frame_rate: u32, size: Option<(u32, u32)>) -> Vec<Property> {
    let (width, height) = size.unwrap_or((1920, 1080));
    let formats: Vec<Id> = formats.iter().map(|format| Id(format.as_raw())).collect();
    vec![
        Property::new(
            FormatProperties::MediaType.as_raw(),
            Value::Id(Id(MediaType::Video.as_raw())),
        ),
        Property::new(
            FormatProperties::MediaSubtype.as_raw(),
            Value::Id(Id(MediaSubtype::Raw.as_raw())),
        ),
        Property::new(
            FormatProperties::VideoFormat.as_raw(),
            Value::Choice(ChoiceValue::Id(Choice(
                ChoiceFlags::empty(),
                ChoiceEnum::Enum {
                    default: formats[0],
                    alternatives: formats,
                },
            ))),
        ),
        Property::new(
            FormatProperties::VideoSize.as_raw(),
            Value::Choice(ChoiceValue::Rectangle(Choice(
                ChoiceFlags::empty(),
                ChoiceEnum::Range {
                    default: Rectangle { width, height },
                    min: Rectangle {
                        width: 1,
                        height: 1,
                    },
                    max: Rectangle {
                        width: LARGEST,
                        height: LARGEST,
                    },
                },
            ))),
        ),
        Property::new(
            FormatProperties::VideoFramerate.as_raw(),
            Value::Choice(ChoiceValue::Fraction(Choice(
                ChoiceFlags::empty(),
                ChoiceEnum::Range {
                    default: Fraction {
                        num: frame_rate,
                        denom: 1,
                    },
                    min: Fraction { num: 0, denom: 1 },
                    max: Fraction {
                        num: 1000,
                        denom: 1,
                    },
                },
            ))),
        ),
    ]
}

pub(crate) fn buffers(dmabuf: bool) -> Vec<u8> {
    let shared = match dmabuf {
        true => 1 << sys::SPA_DATA_DmaBuf,
        false => 1 << sys::SPA_DATA_MemFd | 1 << sys::SPA_DATA_MemPtr,
    };
    object(
        SpaTypes::ObjectParamBuffers,
        ParamType::Buffers,
        vec![
            Property::new(
                sys::SPA_PARAM_BUFFERS_buffers,
                Value::Choice(ChoiceValue::Int(Choice(
                    ChoiceFlags::empty(),
                    ChoiceEnum::Range {
                        default: BUFFERS,
                        min: 2,
                        max: 16,
                    },
                ))),
            ),
            Property::new(sys::SPA_PARAM_BUFFERS_blocks, Value::Int(1)),
            Property::new(
                sys::SPA_PARAM_BUFFERS_dataType,
                Value::Choice(ChoiceValue::Int(Choice(
                    ChoiceFlags::empty(),
                    ChoiceEnum::Flags {
                        default: shared,
                        flags: vec![shared],
                    },
                ))),
            ),
        ],
    )
}

pub(crate) fn metas() -> Vec<Vec<u8>> {
    vec![
        meta(sys::SPA_META_Header, size_of::<sys::spa_meta_header>()),
        meta(sys::SPA_META_VideoCrop, size_of::<sys::spa_meta_region>()),
    ]
}

pub(crate) enum Negotiation {
    Settled(NegotiatedFormat),
    Unsettled,
}

pub(crate) fn read(pod: &Pod) -> Option<Negotiation> {
    let (_, value) = PodDeserializer::deserialize_any_from(pod.as_bytes()).ok()?;
    let Value::Object(object) = value else {
        return None;
    };
    let mut format = None;
    let mut size = None;
    let mut modifier = None;
    for property in &object.properties {
        if property.key == FormatProperties::VideoFormat.as_raw() {
            format = default_id(&property.value).map(VideoFormat::from_raw);
        } else if property.key == FormatProperties::VideoSize.as_raw() {
            size = default_rectangle(&property.value);
        } else if property.key == FormatProperties::VideoModifier.as_raw() {
            let unsettled = property.flags.contains(PropertyFlags::DONT_FIXATE)
                || matches!(
                    &property.value,
                    Value::Choice(ChoiceValue::Long(Choice(
                        _,
                        ChoiceEnum::Enum { alternatives, .. }
                    ))) if alternatives.len() > 1
                );
            if unsettled {
                return Some(Negotiation::Unsettled);
            }
            modifier = match &property.value {
                Value::Long(named) => Some(*named as u64),
                Value::Choice(ChoiceValue::Long(Choice(_, choice))) => match choice {
                    ChoiceEnum::None(named) => Some(*named as u64),
                    ChoiceEnum::Enum { default, .. } => Some(*default as u64),
                    _ => None,
                },
                _ => None,
            };
        }
    }
    let size = size.filter(|size| size.width > 0 && size.height > 0)?;
    let format = format?;
    Some(Negotiation::Settled(NegotiatedFormat {
        format,
        order: order(format)?,
        width: size.width,
        height: size.height,
        modifier,
    }))
}
