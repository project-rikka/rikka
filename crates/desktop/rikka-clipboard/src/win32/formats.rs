use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::{HWND, SetLastError, WIN32_ERROR};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EnumClipboardFormats, GetClipboardData, GetClipboardFormatNameW,
    IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW,
};
use windows::Win32::System::Ole::{
    CF_BITMAP, CF_DIB, CF_DIBV5, CF_HDROP, CF_OEMTEXT, CF_TEXT, CF_UNICODETEXT, CLIPBOARD_FORMAT,
};
use windows::core::HSTRING;

use rikka_clipboard_core::{
    COPIED_FILES, Error, Offer, PNG, Result, SECRET, SECRET_HINT, TEXT, TEXT_TYPES, URI_LIST, dib,
    files_in, hdrop, text_mime, uri_list,
};
use rikka_png::{Decoder, Encoder, Layout, Level, Limits, Metadata, Pixels};

use crate::win32::global::{self, io_error};

const OPEN_ATTEMPTS: u32 = 20;
const OPEN_RETRY: Duration = Duration::from_millis(10);
const NAME_LENGTH: usize = 256;
const PNG_NAME: &str = "PNG";
const DROP_EFFECT_NAME: &str = "Preferred DropEffect";
const EXCLUDE_MONITOR_NAME: &str = "ExcludeClipboardContentFromMonitorProcessing";
const HISTORY_NAME: &str = "CanIncludeInClipboardHistory";
const CLOUD_NAME: &str = "CanUploadToCloudClipboard";
const DROPEFFECT_COPY: u32 = 1;
const DROPEFFECT_MOVE: u32 = 2;
const PICTURE_LIMITS: Limits = Limits {
    max_width: 1 << 15,
    max_height: 1 << 15,
    max_bytes: 1 << 30,
};

pub(super) struct Opened;

impl Opened {
    pub(super) fn new(window: HWND) -> Result<Opened> {
        let mut attempt = 0;
        loop {
            match unsafe { OpenClipboard(Some(window)) } {
                Ok(()) => return Ok(Opened),
                Err(_) if attempt + 1 < OPEN_ATTEMPTS => {
                    attempt += 1;
                    thread::sleep(OPEN_RETRY);
                }
                Err(error) => return Err(io_error(error)),
            }
        }
    }
}

impl Drop for Opened {
    fn drop(&mut self) {
        if let Err(error) = unsafe { CloseClipboard() } {
            tracing::debug!("failed to close clipboard: {error}");
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Registered {
    png: u32,
    drop_effect: u32,
    exclude_monitor: u32,
    history: u32,
    cloud: u32,
}

impl Registered {
    pub(super) fn new() -> Registered {
        Registered {
            png: register(PNG_NAME),
            drop_effect: register(DROP_EFFECT_NAME),
            exclude_monitor: register(EXCLUDE_MONITOR_NAME),
            history: register(HISTORY_NAME),
            cloud: register(CLOUD_NAME),
        }
    }

    fn secret(&self) -> [u32; 3] {
        [self.exclude_monitor, self.history, self.cloud]
    }
}

fn register(name: &str) -> u32 {
    let format = unsafe { RegisterClipboardFormatW(&HSTRING::from(name)) };
    if format == 0 {
        tracing::warn!(
            "failed to register clipboard format `{name}`: {}",
            windows::core::Error::from_thread()
        );
    }
    format
}

fn standard(format: CLIPBOARD_FORMAT) -> u32 {
    u32::from(format.0)
}

fn is_text(mime: &str) -> bool {
    TEXT_TYPES.contains(&mime)
}

pub(super) fn publishable(offer: &Offer, registered: &Registered) -> Vec<u32> {
    let mut formats = Vec::new();
    let mimes: Vec<&str> = offer.mime_types().collect();
    if mimes.iter().any(|mime| is_text(mime)) {
        formats.push(standard(CF_UNICODETEXT));
    }
    if let Some(uris) = offer.data(URI_LIST)
        && hdrop::encode(&uri_list(&String::from_utf8_lossy(&uris))).is_some()
    {
        formats.push(standard(CF_HDROP));
        formats.push(registered.drop_effect);
    }
    if mimes.contains(&PNG) {
        formats.push(registered.png);
        formats.push(standard(CF_DIBV5));
    }
    if mimes.contains(&SECRET_HINT) {
        formats.extend(registered.secret());
    }
    for mime in mimes {
        if is_text(mime) || mime == PNG || mime == SECRET_HINT {
            continue;
        }
        formats.push(register(mime));
    }
    let mut unique = Vec::with_capacity(formats.len());
    for format in formats {
        if format != 0 && !unique.contains(&format) {
            unique.push(format);
        }
    }
    unique
}

pub(super) fn render(offer: &Offer, format: u32, registered: &Registered) -> Option<Vec<u8>> {
    let mimes: Vec<String> = offer.mime_types().map(str::to_owned).collect();
    if format == standard(CF_UNICODETEXT) {
        let text = offer.data(text_mime(&mimes)?)?;
        return Some(wide_bytes(&String::from_utf8_lossy(&text)));
    }
    if format == standard(CF_HDROP) {
        let uris = offer.data(URI_LIST)?;
        return hdrop::encode(&uri_list(&String::from_utf8_lossy(&uris)));
    }
    if format == registered.drop_effect {
        let files = files_in(&mimes, |mime| offer.data(mime).map(|data| data.to_vec()))?;
        let effect = match files.cut {
            true => DROPEFFECT_MOVE,
            false => DROPEFFECT_COPY,
        };
        return Some(effect.to_le_bytes().to_vec());
    }
    if format == registered.png {
        return offer.data(PNG).map(|data| data.to_vec());
    }
    if format == standard(CF_DIBV5) {
        let png = offer.data(PNG)?;
        let mut pixels = Vec::new();
        let (width, height) = Decoder::new()
            .rgba8(&png, &PICTURE_LIMITS, &mut pixels)
            .inspect_err(|error| tracing::debug!("cannot decode clipboard picture: {error}"))
            .ok()?;
        return dib::encode_v5(&dib::Rgba {
            width,
            height,
            pixels,
        });
    }
    if registered.secret().contains(&format) {
        return Some(0u32.to_le_bytes().to_vec());
    }
    let mime = format_name(format)?;
    offer.data(&mime).map(|data| data.to_vec())
}

pub(super) fn offered(registered: &Registered) -> Vec<String> {
    let mut mimes = Vec::new();
    let mut add = |mime: &str| {
        if !mimes.iter().any(|known: &String| known == mime) {
            mimes.push(mime.to_owned());
        }
    };
    let mut format = 0;
    loop {
        format = unsafe { EnumClipboardFormats(format) };
        if format == 0 {
            break;
        }
        match format {
            _ if [CF_UNICODETEXT, CF_TEXT, CF_OEMTEXT]
                .into_iter()
                .any(|text| standard(text) == format) =>
            {
                add(TEXT);
                add("text/plain");
            }
            _ if format == standard(CF_HDROP) => {
                add(URI_LIST);
                add(COPIED_FILES);
            }
            _ if format == registered.png
                || [CF_DIB, CF_DIBV5, CF_BITMAP]
                    .into_iter()
                    .any(|picture| standard(picture) == format) =>
            {
                add(PNG);
            }
            _ if format == registered.exclude_monitor => add(SECRET_HINT),
            _ => {
                if let Some(name) = format_name(format).filter(|name| name.contains('/')) {
                    add(&name);
                }
            }
        }
    }
    mimes
}

pub(super) fn read(mime: &str, limit: usize, registered: &Registered) -> Result<Vec<u8>> {
    if is_text(mime) {
        let bytes = data(standard(CF_UNICODETEXT), limit.saturating_mul(2))?;
        return Ok(narrow(&bytes).into_bytes());
    }
    if let Some(named) = available_named(mime) {
        return data(named, limit);
    }
    match mime {
        URI_LIST => Ok(dropped_uris(limit)?.join("\r\n").into_bytes()),
        COPIED_FILES => {
            let uris = dropped_uris(limit)?;
            let verb = match data(registered.drop_effect, limit) {
                Ok(effect) if effect.get(..4) == Some(&DROPEFFECT_MOVE.to_le_bytes()[..]) => "cut",
                _ => "copy",
            };
            let lines: Vec<&str> = std::iter::once(verb)
                .chain(uris.iter().map(String::as_str))
                .collect();
            Ok(lines.join("\n").into_bytes())
        }
        PNG => read_png(limit, registered),
        SECRET_HINT => match available(registered.exclude_monitor) {
            true => Ok(SECRET.as_bytes().to_vec()),
            false => Err(Error::NoSuchType(mime.to_owned())),
        },
        _ => Err(Error::NoSuchType(mime.to_owned())),
    }
}

fn read_png(limit: usize, registered: &Registered) -> Result<Vec<u8>> {
    if available(registered.png) {
        return data(registered.png, limit);
    }
    let format = [CF_DIBV5, CF_DIB]
        .into_iter()
        .map(standard)
        .find(|format| available(*format))
        .ok_or_else(|| Error::NoSuchType(PNG.to_owned()))?;
    let bytes = data(format, usize::MAX)?;
    let picture = dib::decode(&bytes).ok_or(Error::Unavailable("unsupported clipboard bitmap"))?;
    let mut png = Vec::new();
    Encoder::new(Level::Fast)
        .encode(
            Pixels {
                width: picture.width,
                height: picture.height,
                layout: Layout::Rgba8,
                bytes: &picture.pixels,
            },
            &Metadata::default(),
            &mut png,
        )
        .map_err(|error| Error::Io(std::io::Error::other(error)))?;
    if png.len() > limit {
        return Err(Error::Io(std::io::Error::other(format!(
            "clipboard picture of {} bytes exceeds {limit}",
            png.len()
        ))));
    }
    Ok(png)
}

fn dropped_uris(limit: usize) -> Result<Vec<String>> {
    let bytes = data(standard(CF_HDROP), limit)?;
    Ok(hdrop::decode(&bytes))
}

fn available(format: u32) -> bool {
    format != 0 && unsafe { IsClipboardFormatAvailable(format) }.is_ok()
}

fn available_named(mime: &str) -> Option<u32> {
    if !mime.contains('/') {
        return None;
    }
    let format = register(mime);
    available(format).then_some(format)
}

fn data(format: u32, limit: usize) -> Result<Vec<u8>> {
    if !available(format) {
        return Err(Error::Empty);
    }
    unsafe { SetLastError(WIN32_ERROR(0)) };
    let handle = unsafe { GetClipboardData(format) }.map_err(io_error)?;
    global::read(handle, limit)
}

fn format_name(format: u32) -> Option<String> {
    let mut name = [0u16; NAME_LENGTH];
    let length = unsafe { GetClipboardFormatNameW(format, &mut name) };
    let length = usize::try_from(length).ok().filter(|length| *length > 0)?;
    Some(String::from_utf16_lossy(&name[..length]))
}

fn wide_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain([0])
        .flat_map(u16::to_le_bytes)
        .collect()
}

fn narrow(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .take_while(|unit| *unit != 0)
        .collect();
    String::from_utf16_lossy(&units)
}
