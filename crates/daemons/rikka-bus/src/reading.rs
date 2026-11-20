use zbus::zvariant::Value;

pub(crate) const NULL_OBJECT_PATH: &str = "/";

pub(crate) fn text<'a>(value: &'a Value<'a>) -> Option<&'a str> {
    match unwrapped(value) {
        Value::Str(text) => Some(text.as_str()),
        Value::ObjectPath(path) => Some(path.as_str()),
        _ => None,
    }
}

pub(crate) fn object_path<'a>(value: &'a Value<'a>) -> Option<&'a str> {
    match unwrapped(value) {
        Value::ObjectPath(path) if path.as_str() != NULL_OBJECT_PATH => Some(path.as_str()),
        _ => None,
    }
}

pub(crate) fn int64(value: &Value<'_>) -> Option<i64> {
    match unwrapped(value) {
        Value::I64(number) => Some(*number),
        Value::U64(number) => i64::try_from(*number).ok(),
        Value::I32(number) => Some(i64::from(*number)),
        Value::U32(number) => Some(i64::from(*number)),
        Value::I16(number) => Some(i64::from(*number)),
        Value::U16(number) => Some(i64::from(*number)),
        Value::U8(number) => Some(i64::from(*number)),
        Value::F64(number) if number.is_finite() => Some(*number as i64),
        _ => None,
    }
}

pub(crate) fn uint64(value: &Value<'_>) -> Option<u64> {
    match unwrapped(value) {
        Value::U64(number) => Some(*number),
        _ => u64::try_from(int64(value)?).ok(),
    }
}

pub(crate) fn double(value: &Value<'_>) -> Option<f64> {
    match unwrapped(value) {
        Value::F64(number) if number.is_finite() => Some(*number),
        _ => int64(value).map(|number| number as f64),
    }
}

pub(crate) fn boolean(value: &Value<'_>) -> Option<bool> {
    match unwrapped(value) {
        Value::Bool(flag) => Some(*flag),
        _ => None,
    }
}

pub(crate) fn texts(value: &Value<'_>) -> Vec<String> {
    match unwrapped(value) {
        Value::Array(array) => array
            .iter()
            .filter_map(text)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
            .collect(),
        Value::Str(text) if !text.is_empty() => vec![text.as_str().to_owned()],
        _ => Vec::new(),
    }
}

fn unwrapped<'a>(value: &'a Value<'a>) -> &'a Value<'a> {
    match value {
        Value::Value(inner) => unwrapped(inner),
        other => other,
    }
}
