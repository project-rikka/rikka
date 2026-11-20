use rikka_model_core::Endian;

use crate::file::Record;

const INT: i64 = 1;
const FLOAT: i64 = 2;
const GROUP: i64 = 6;
const DOUBLE: i64 = 8;
const BOOLEAN: i64 = 10;

pub(crate) fn number(property: &Record<'_>) -> Option<f64> {
    let data = property.record("data")?;
    let low = data.int("val").unwrap_or(0) as u32;
    let high = data.int("val2").unwrap_or(0) as u32;
    match property.int("type")? {
        INT | BOOLEAN => Some(f64::from(low as i32)),
        FLOAT => Some(f64::from(f32::from_bits(low))),
        DOUBLE => {
            let bits = match property.blend().endian() {
                Endian::Little => u64::from(low) | (u64::from(high) << 32),
                Endian::Big => (u64::from(low) << 32) | u64::from(high),
            };
            Some(f64::from_bits(bits))
        }
        _ => None,
    }
}

pub(crate) fn child<'a>(group: &Record<'a>, name: &str) -> Option<Record<'a>> {
    if group.int("type") != Some(GROUP) {
        return None;
    }
    group
        .record("data")?
        .list("group")
        .into_iter()
        .find(|child| child.text("name").as_deref() == Some(name))
}
