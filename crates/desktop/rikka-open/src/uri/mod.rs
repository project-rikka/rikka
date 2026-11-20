#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::{decode_path, encode_path, path_to_uri, uri_to_path};
#[cfg(windows)]
pub use windows::{decode_path, encode_path, path_to_uri, uri_to_path};

use percent_encoding::{AsciiSet, CONTROLS};

const ESCAPED: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

fn split_file_uri(uri: &str) -> Option<(&str, &str)> {
    let (scheme, rest) = uri.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("file") {
        return None;
    }
    rest.split_once('/')
}

fn is_local(authority: &str) -> bool {
    authority.is_empty() || authority.eq_ignore_ascii_case("localhost")
}
