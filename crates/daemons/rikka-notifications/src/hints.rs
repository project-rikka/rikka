use std::collections::HashMap;

use zbus::zvariant::{OwnedValue, Value};

use crate::image::{self, Image, RawPicture};
use crate::progress::Progress;
use crate::urgency::Urgency;

#[derive(Debug, Default)]
pub(crate) struct Hints {
    pub(crate) urgency: Urgency,
    pub(crate) desktop_entry: Option<String>,
    pub(crate) image: Option<Image>,
    pub(crate) resident: bool,
    pub(crate) action_icons: bool,
    pub(crate) transient: bool,
    pub(crate) progress: Option<Progress>,
}

const PIXELS: [&str; 3] = ["image-data", "image_data", "icon_data"];
const PATHS: [&str; 2] = ["image-path", "image_path"];

impl Hints {
    #[must_use]
    pub(crate) fn read(raw: &HashMap<String, OwnedValue>) -> Hints {
        let named = |key: &str| raw.get(key).map(|value| inner(value));
        Hints {
            urgency: named("urgency")
                .and_then(level)
                .map_or_else(Urgency::default, Urgency::level),
            desktop_entry: named("desktop-entry").and_then(text),
            image: picture(raw).or_else(|| path(raw)),
            resident: named("resident").and_then(flag).unwrap_or_default(),
            action_icons: named("action-icons").and_then(flag).unwrap_or_default(),
            transient: named("transient").and_then(flag).unwrap_or_default(),
            progress: named("value").and_then(number).and_then(Progress::percent),
        }
    }
}

fn picture(raw: &HashMap<String, OwnedValue>) -> Option<Image> {
    let found = PIXELS
        .iter()
        .find_map(|key| raw.get(*key).and_then(|value| pixels(inner(value))))?;
    let decoded = image::decode(&found);
    if decoded.is_none() {
        tracing::debug!("notification image-data is malformed");
    }
    decoded.map(Image::Pixels)
}

fn path(raw: &HashMap<String, OwnedValue>) -> Option<Image> {
    PATHS
        .iter()
        .find_map(|key| raw.get(*key).and_then(|value| text(inner(value))))
        .and_then(|named| Image::named(&named))
}

fn pixels(value: &Value<'_>) -> Option<RawPicture> {
    let Value::Structure(structure) = value else {
        return None;
    };
    let fields = structure.fields();
    let [width, height, rowstride, has_alpha, depth, channels, data] = fields else {
        return None;
    };
    flag(inner(has_alpha))?;
    Some(RawPicture {
        width: number(inner(width))?,
        height: number(inner(height))?,
        rowstride: number(inner(rowstride))?,
        bits_per_sample: number(inner(depth))?,
        channels: number(inner(channels))?,
        data: bytes(inner(data))?,
    })
}

fn bytes(value: &Value<'_>) -> Option<Vec<u8>> {
    let Value::Array(array) = value else {
        return None;
    };
    array
        .inner()
        .iter()
        .map(|element| match inner(element) {
            Value::U8(byte) => Some(*byte),
            _ => None,
        })
        .collect()
}

fn inner<'a>(value: &'a Value<'a>) -> &'a Value<'a> {
    match value {
        Value::Value(nested) => inner(nested),
        plain => plain,
    }
}

fn text(value: &Value<'_>) -> Option<String> {
    match value {
        Value::Str(text) => Some(text.to_string()),
        _ => None,
    }
}

fn flag(value: &Value<'_>) -> Option<bool> {
    match value {
        Value::Bool(flag) => Some(*flag),
        _ => number(value).map(|number| number != 0),
    }
}

fn level(value: &Value<'_>) -> Option<u8> {
    number(value).and_then(|number| u8::try_from(number).ok())
}

fn number(value: &Value<'_>) -> Option<i32> {
    match value {
        Value::U8(number) => Some(i32::from(*number)),
        Value::I16(number) => Some(i32::from(*number)),
        Value::U16(number) => Some(i32::from(*number)),
        Value::I32(number) => Some(*number),
        Value::U32(number) => i32::try_from(*number).ok(),
        Value::I64(number) => i32::try_from(*number).ok(),
        Value::U64(number) => i32::try_from(*number).ok(),
        _ => None,
    }
}
