use jiff::Timestamp;
use jiff::tz::TimeZone;

#[must_use]
pub fn format_timestamp(seconds: i64) -> String {
    let Ok(timestamp) = Timestamp::from_second(seconds) else {
        return String::new();
    };
    timestamp
        .to_zoned(TimeZone::system())
        .strftime("%Y/%m/%d %H:%M")
        .to_string()
}
