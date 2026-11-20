use zbus::zvariant::{OwnedValue, Value};

use crate::protocol::CastSource;

const VENDOR: &str = "Rikka";

const VERSION: u32 = 1;

pub(crate) fn read(restore_data: &(String, u32, OwnedValue)) -> Vec<CastSource> {
    let (vendor, version, data) = restore_data;
    if vendor != VENDOR || *version != VERSION {
        return Vec::new();
    }
    let Ok(json) = <&str>::try_from(data) else {
        return Vec::new();
    };
    serde_json::from_str(json).unwrap_or_default()
}

pub(crate) fn write(sources: &[CastSource]) -> (String, u32, OwnedValue) {
    let json = serde_json::to_string(sources).expect("cast sources serialize to JSON");
    let data =
        OwnedValue::try_from(Value::new(json)).expect("a string value holds no file descriptor");
    (VENDOR.to_owned(), VERSION, data)
}
