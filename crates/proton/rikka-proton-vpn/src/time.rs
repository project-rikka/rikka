use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};

const HTTP_DATE: &str = "%a, %d %b %Y %H:%M:%S GMT";

pub(crate) fn http_date(at: Timestamp) -> String {
    Zoned::new(at, TimeZone::UTC)
        .strftime(HTTP_DATE)
        .to_string()
}

pub(crate) fn parse_http_date(text: &str) -> Option<Timestamp> {
    jiff::fmt::rfc2822::parse(text.trim())
        .map(|zoned| zoned.timestamp())
        .ok()
}
