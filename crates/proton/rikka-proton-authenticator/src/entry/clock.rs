pub(crate) fn counter(at: jiff::Timestamp, period: u16) -> u64 {
    let period = i64::from(period.max(1));
    at.as_second().div_euclid(period) as u64
}

pub(crate) fn remaining(at: jiff::Timestamp, period: u16) -> u16 {
    let period = i64::from(period.max(1));
    (period - at.as_second().rem_euclid(period)) as u16
}
