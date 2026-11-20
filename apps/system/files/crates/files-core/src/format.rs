use std::time::{SystemTime, UNIX_EPOCH};

#[must_use]
pub fn permissions(mode: u32) -> String {
    let bit = |shift: u32, flag: u32, letter: char| {
        if mode >> shift & flag != 0 {
            letter
        } else {
            '-'
        }
    };
    let mut out = String::with_capacity(9);
    for shift in [6, 3, 0] {
        out.push(bit(shift, 0b100, 'r'));
        out.push(bit(shift, 0b010, 'w'));
        out.push(bit(shift, 0b001, 'x'));
    }
    format!("{out} ({:03o})", mode & 0o777)
}

#[must_use]
pub fn timestamp(time: Option<SystemTime>) -> String {
    let Some(time) = time else {
        return String::new();
    };
    let seconds = match time.duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_secs()).unwrap_or(i64::MAX),
        Err(before) => {
            i64::try_from(before.duration().as_secs()).map_or(i64::MIN, |seconds| -seconds)
        }
    };
    rikka_format::format_timestamp(seconds)
}
