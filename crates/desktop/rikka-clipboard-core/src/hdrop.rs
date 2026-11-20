use percent_encoding::{AsciiSet, CONTROLS, percent_decode_str, utf8_percent_encode};

const HEADER: usize = 20;
const FILE_SCHEME: &str = "file://";

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

#[must_use]
pub fn encode(uris: &[String]) -> Option<Vec<u8>> {
    let paths: Vec<String> = uris.iter().filter_map(|uri| path_of(uri)).collect();
    if paths.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    out.extend_from_slice(&(HEADER as u32).to_le_bytes());
    out.extend_from_slice(&[0; 12]);
    out.extend_from_slice(&1u32.to_le_bytes());
    for path in &paths {
        for unit in path.encode_utf16().chain([0]) {
            out.extend_from_slice(&unit.to_le_bytes());
        }
    }
    out.extend_from_slice(&[0, 0]);
    Some(out)
}

#[must_use]
pub fn decode(bytes: &[u8]) -> Vec<String> {
    let Some(start) = bytes
        .get(..4)
        .and_then(|raw| raw.try_into().ok())
        .map(|raw| u32::from_le_bytes(raw) as usize)
    else {
        return Vec::new();
    };
    let wide = bytes
        .get(16..20)
        .and_then(|raw| raw.try_into().ok())
        .is_some_and(|raw| u32::from_le_bytes(raw) != 0);
    let Some(list) = bytes.get(start..) else {
        return Vec::new();
    };
    let names: Vec<String> = match wide {
        true => {
            let units: Vec<u16> = list
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_le_bytes(*pair))
                .collect();
            units
                .split(|unit| *unit == 0)
                .take_while(|name| !name.is_empty())
                .map(String::from_utf16_lossy)
                .collect()
        }
        false => list
            .split(|byte| *byte == 0)
            .take_while(|name| !name.is_empty())
            .map(|name| String::from_utf8_lossy(name).into_owned())
            .collect(),
    };
    names.iter().map(|path| uri_of(path)).collect()
}

#[must_use]
pub fn uri_of(path: &str) -> String {
    let slashed = path.replace('\\', "/");
    match slashed.strip_prefix("//") {
        Some(unc) => format!("{FILE_SCHEME}{}", utf8_percent_encode(unc, ESCAPED)),
        None => format!("{FILE_SCHEME}/{}", utf8_percent_encode(&slashed, ESCAPED)),
    }
}

#[must_use]
pub fn path_of(uri: &str) -> Option<String> {
    let (scheme, rest) = uri.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("file") {
        return None;
    }
    let (authority, path) = rest.split_once('/')?;
    let decoded = percent_decode_str(path).decode_utf8().ok()?;
    let local = authority.is_empty() || authority.eq_ignore_ascii_case("localhost");
    let path = match local {
        true => decoded.into_owned(),
        false => format!("//{authority}/{decoded}"),
    };
    Some(path.replace('/', "\\"))
}
