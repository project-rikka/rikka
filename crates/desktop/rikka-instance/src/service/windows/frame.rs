use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt as _, OsStringExt as _};
use std::path::PathBuf;

use crate::launch_request::LaunchRequest;

pub(super) const HEADER: usize = 4;
pub(super) const LIMIT: usize = 1 << 20;

const ARGUMENT: u8 = 1;
const URI: u8 = 2;
const ACTION: u8 = 3;
const CWD: u8 = 4;

pub(super) fn encode(request: &LaunchRequest) -> Vec<u8> {
    let mut body = Vec::new();
    for argument in request.arguments() {
        field(&mut body, ARGUMENT, &wide(argument));
    }
    for uri in request.uris() {
        field(&mut body, URI, uri.as_bytes());
    }
    if let Some(action) = request.action() {
        field(&mut body, ACTION, action.as_bytes());
    }
    if let Some(cwd) = request.cwd() {
        field(&mut body, CWD, &wide(cwd.as_os_str()));
    }
    let length = u32::try_from(body.len()).unwrap_or(u32::MAX);
    let mut frame = length.to_le_bytes().to_vec();
    frame.extend(body);
    frame
}

pub(super) fn length(header: [u8; HEADER]) -> usize {
    u32::from_le_bytes(header) as usize
}

pub(super) fn decode(mut body: &[u8]) -> Option<LaunchRequest> {
    let mut arguments = Vec::new();
    let mut uris = Vec::new();
    let mut action = None;
    let mut cwd = None;
    while let [kind, rest @ ..] = body {
        let (size, rest) = rest.split_first_chunk::<HEADER>()?;
        let size = u32::from_le_bytes(*size) as usize;
        let value = rest.get(..size)?;
        body = &rest[size..];
        match *kind {
            ARGUMENT => arguments.push(from_wide(value)?),
            URI => uris.push(String::from_utf8(value.to_vec()).ok()?),
            ACTION => action = Some(String::from_utf8(value.to_vec()).ok()?),
            CWD => cwd = Some(PathBuf::from(from_wide(value)?)).filter(|path| path.is_absolute()),
            _ => return None,
        }
    }
    let request = match (action, arguments.is_empty(), uris.is_empty()) {
        (Some(action), _, _) => LaunchRequest::from_action(action),
        (None, false, _) => return Some(LaunchRequest::from_command_line(arguments, cwd)),
        (None, true, false) => LaunchRequest::from_uris(uris),
        (None, true, true) => LaunchRequest::default(),
    };
    Some(match cwd {
        Some(cwd) => request.with_cwd(cwd),
        None => request,
    })
}

fn field(body: &mut Vec<u8>, kind: u8, value: &[u8]) {
    body.push(kind);
    body.extend(u32::try_from(value.len()).unwrap_or(u32::MAX).to_le_bytes());
    body.extend_from_slice(value);
}

fn wide(text: &OsStr) -> Vec<u8> {
    text.encode_wide().flat_map(u16::to_le_bytes).collect()
}

fn from_wide(bytes: &[u8]) -> Option<OsString> {
    let (units, rest) = bytes.as_chunks::<2>();
    if !rest.is_empty() {
        return None;
    }
    let units: Vec<u16> = units.iter().map(|unit| u16::from_le_bytes(*unit)).collect();
    Some(OsString::from_wide(&units))
}
