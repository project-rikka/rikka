use libspa::param::ParamType;
use libspa::param::format::{FormatProperties, MediaSubtype, MediaType};
use libspa::param::video::VideoFormat;
use libspa::pod::deserialize::PodDeserializer;
use libspa::pod::{ChoiceValue, Pod, Property, PropertyFlags, Value};
use libspa::sys;
use libspa::utils::{Choice, ChoiceEnum, ChoiceFlags, Fraction, Id, Rectangle, SpaTypes};
use rikka_capture::{Constraints, PixelFormat};
use rikka_pipewire::{default_id, default_rectangle, meta, object};

const BUFFERS: i32 = 4;
const BUFFERS_MIN: i32 = 2;
const BUFFERS_MAX: i32 = 16;

pub(crate) const CURSOR_EXTENT: u32 = 256;

#[derive(Clone, Debug)]
pub(crate) struct Negotiated {
    pub(crate) format: PixelFormat,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) modifier: Option<u64>,
    pub(crate) modifier_choices: Vec<u64>,
}

impl Negotiated {
    pub(crate) fn stride(&self) -> u32 {
        self.width * 4
    }

    pub(crate) fn frame_size(&self) -> u32 {
        self.stride() * self.height
    }
}

pub(crate) fn offers(constraints: &Constraints) -> Vec<Vec<u8>> {
    let mut offered = Vec::new();
    if let Some(dmabuf) = &constraints.dmabuf {
        for format in PixelFormat::ALL {
            let modifiers = dmabuf.modifiers(format);
            if modifiers.is_empty() {
                continue;
            }
            offered.push(format_pod(
                format,
                Some((modifiers, true)),
                constraints.width,
                constraints.height,
            ));
        }
    }
    offered.extend(shared(constraints));
    offered
}

fn shared(constraints: &Constraints) -> Vec<Vec<u8>> {
    PixelFormat::ALL
        .into_iter()
        .filter(|format| constraints.shm.contains(format))
        .map(|format| format_pod(format, None, constraints.width, constraints.height))
        .collect()
}

pub(crate) fn fixate_modifier(
    negotiated: &Negotiated,
    modifier: u64,
    constraints: &Constraints,
) -> Vec<Vec<u8>> {
    let mut offered = vec![format_pod(
        negotiated.format,
        Some((&[modifier], false)),
        negotiated.width,
        negotiated.height,
    )];
    offered.extend(shared(constraints));
    offered
}

pub(crate) fn read(pod: &Pod) -> Option<Negotiated> {
    let (_, value) = PodDeserializer::deserialize_any_from(pod.as_bytes()).ok()?;
    let Value::Object(object) = value else {
        return None;
    };

    let mut format = None;
    let mut width = 0;
    let mut height = 0;
    let mut modifier = None;
    let mut modifier_choices = Vec::new();

    for property in &object.properties {
        if property.key == FormatProperties::VideoFormat.as_raw() {
            format = default_id(&property.value).and_then(pixel_format);
        } else if property.key == FormatProperties::VideoSize.as_raw() {
            if let Some(size) = default_rectangle(&property.value) {
                width = size.width;
                height = size.height;
            }
        } else if property.key == FormatProperties::VideoModifier.as_raw() {
            let named = long_values(&property.value);
            modifier = named.first().copied();
            if named.len() > 1 || property.flags.contains(PropertyFlags::DONT_FIXATE) {
                modifier_choices = named;
            }
        }
    }

    Some(Negotiated {
        format: format?,
        width,
        height,
        modifier,
        modifier_choices,
    })
}

fn long_values(value: &Value) -> Vec<u64> {
    match value {
        Value::Long(named) => vec![*named as u64],
        Value::Choice(ChoiceValue::Long(Choice(_, choice))) => {
            let mut named = choices(choice);
            named.dedup();
            named
        }
        _ => Vec::new(),
    }
}

pub(crate) fn buffers(negotiated: &Negotiated, dmabuf: bool) -> Vec<u8> {
    let kind = match dmabuf {
        true => sys::SPA_DATA_DmaBuf,
        false => sys::SPA_DATA_MemFd,
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
                        min: BUFFERS_MIN,
                        max: BUFFERS_MAX,
                    },
                ))),
            ),
            Property::new(sys::SPA_PARAM_BUFFERS_blocks, Value::Int(1)),
            Property::new(
                sys::SPA_PARAM_BUFFERS_size,
                Value::Int(negotiated.frame_size() as i32),
            ),
            Property::new(
                sys::SPA_PARAM_BUFFERS_stride,
                Value::Int(negotiated.stride() as i32),
            ),
            Property::new(sys::SPA_PARAM_BUFFERS_align, Value::Int(16)),
            Property::new(
                sys::SPA_PARAM_BUFFERS_dataType,
                Value::Choice(ChoiceValue::Int(Choice(
                    ChoiceFlags::empty(),
                    ChoiceEnum::Flags {
                        default: 1 << kind,
                        flags: vec![1 << kind],
                    },
                ))),
            ),
        ],
    )
}

pub(crate) fn metas(cursor: bool) -> Vec<Vec<u8>> {
    let mut metas = vec![
        meta(sys::SPA_META_Header, size_of::<sys::spa_meta_header>()),
        meta(
            sys::SPA_META_VideoDamage,
            size_of::<sys::spa_meta_region>() * DAMAGE_REGIONS,
        ),
    ];
    if cursor {
        metas.push(meta(sys::SPA_META_Cursor, cursor_meta_size()));
    }
    metas
}

pub(crate) const DAMAGE_REGIONS: usize = 8;

pub(crate) fn cursor_meta_size() -> usize {
    size_of::<sys::spa_meta_cursor>()
        + size_of::<sys::spa_meta_bitmap>()
        + (CURSOR_EXTENT * CURSOR_EXTENT * 4) as usize
}

pub(crate) fn video_format(format: PixelFormat) -> VideoFormat {
    match format {
        PixelFormat::Xrgb8888 => VideoFormat::BGRx,
        PixelFormat::Argb8888 => VideoFormat::BGRA,
        PixelFormat::Xbgr8888 => VideoFormat::RGBx,
        PixelFormat::Abgr8888 => VideoFormat::RGBA,
    }
}

fn pixel_format(id: u32) -> Option<PixelFormat> {
    PixelFormat::ALL
        .into_iter()
        .find(|format| video_format(*format).as_raw() == id)
}

fn choices(choice: &ChoiceEnum<i64>) -> Vec<u64> {
    match choice {
        ChoiceEnum::None(only) => vec![*only as u64],
        ChoiceEnum::Enum {
            default,
            alternatives,
        } => std::iter::once(*default)
            .chain(alternatives.iter().copied())
            .map(|modifier| modifier as u64)
            .collect(),
        ChoiceEnum::Flags { default, flags } => std::iter::once(*default)
            .chain(flags.iter().copied())
            .map(|modifier| modifier as u64)
            .collect(),
        ChoiceEnum::Range { default, .. } | ChoiceEnum::Step { default, .. } => {
            vec![*default as u64]
        }
    }
}

fn format_pod(
    format: PixelFormat,
    modifiers: Option<(&[u64], bool)>,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let mut properties = vec![
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
            Value::Id(Id(video_format(format).as_raw())),
        ),
        Property::new(
            FormatProperties::VideoSize.as_raw(),
            Value::Rectangle(Rectangle { width, height }),
        ),
        Property::new(
            FormatProperties::VideoFramerate.as_raw(),
            Value::Fraction(Fraction { num: 0, denom: 1 }),
        ),
    ];

    if let Some((modifiers, unsettled)) = modifiers.filter(|(named, _)| !named.is_empty()) {
        let mut flags = PropertyFlags::MANDATORY;
        if unsettled {
            flags |= PropertyFlags::DONT_FIXATE;
        }
        let named: Vec<i64> = modifiers.iter().map(|modifier| *modifier as i64).collect();
        properties.push(Property {
            key: FormatProperties::VideoModifier.as_raw(),
            flags,
            value: Value::Choice(ChoiceValue::Long(Choice(
                ChoiceFlags::empty(),
                ChoiceEnum::Enum {
                    default: named[0],
                    alternatives: named,
                },
            ))),
        });
    }

    object(
        SpaTypes::ObjectParamFormat,
        ParamType::EnumFormat,
        properties,
    )
}
