use std::str::Utf8Error;

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};

const NOT_UNRESERVED: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

#[must_use]
pub fn encode(value: &str) -> String {
    utf8_percent_encode(value, NOT_UNRESERVED).to_string()
}

pub fn decode(value: &str) -> Result<String, Utf8Error> {
    percent_decode_str(value)
        .decode_utf8()
        .map(|decoded| decoded.into_owned())
}
