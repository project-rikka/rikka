use std::sync::Arc;

use rikka_clipboard_core::{Offer, TEXT, TEXT_TYPES, URI_LIST, hdrop, text_mime, uri_list};
use windows::Win32::Foundation::{GlobalFree, HGLOBAL, S_OK};
use windows::Win32::System::Com::{
    DATADIR_GET, DVASPECT_CONTENT, FORMATETC, IDataObject, TYMED_HGLOBAL,
};
use windows::Win32::System::DataExchange::{GetClipboardFormatNameW, RegisterClipboardFormatW};
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows::Win32::System::Ole::{CF_HDROP, CF_UNICODETEXT, ReleaseStgMedium};
use windows::core::HSTRING;

const OWNER_NAME: &str = "Rikka Drag Owner";
const NAME_LENGTH: usize = 256;
const ENUMERATED: usize = 32;

pub(crate) struct Entry {
    pub(crate) format: u16,
    pub(crate) bytes: Arc<[u8]>,
}

pub(crate) fn register(name: &str) -> u16 {
    let format = unsafe { RegisterClipboardFormatW(&HSTRING::from(name)) };
    u16::try_from(format).unwrap_or(0)
}

pub(crate) fn owner_format() -> u16 {
    register(OWNER_NAME)
}

pub(crate) fn describe(format: u16) -> FORMATETC {
    FORMATETC {
        cfFormat: format,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    }
}

pub(crate) fn entries(offer: &Offer) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mimes: Vec<String> = offer.mime_types().map(str::to_owned).collect();
    if let Some(text) = text_mime(&mimes).and_then(|mime| offer.data(mime)) {
        let wide: Vec<u8> = String::from_utf8_lossy(&text)
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect();
        entries.push(Entry {
            format: CF_UNICODETEXT.0,
            bytes: Arc::from(wide),
        });
    }
    if let Some(uris) = offer.data(URI_LIST)
        && let Some(dropped) = hdrop::encode(&uri_list(&String::from_utf8_lossy(&uris)))
    {
        entries.push(Entry {
            format: CF_HDROP.0,
            bytes: Arc::from(dropped),
        });
    }
    for (mime, bytes) in offer.entries() {
        if TEXT_TYPES.contains(&mime.as_str()) {
            continue;
        }
        let format = register(mime);
        if format != 0 {
            entries.push(Entry {
                format,
                bytes: bytes.clone(),
            });
        }
    }
    let owner = owner_format();
    if owner != 0 {
        entries.push(Entry {
            format: owner,
            bytes: Arc::from(std::process::id().to_le_bytes()),
        });
    }
    entries
}

pub(crate) fn is_ours(data: &IDataObject) -> bool {
    let owner = owner_format();
    owner != 0
        && read_format(data, owner, size_of::<u32>())
            .is_some_and(|bytes| bytes.get(..4) == Some(&std::process::id().to_le_bytes()[..]))
}

pub(crate) fn mimes(data: &IDataObject) -> Vec<String> {
    let mut mimes: Vec<String> = Vec::new();
    let mut add = |mime: &str| {
        if !mimes.iter().any(|known| known == mime) {
            mimes.push(mime.to_owned());
        }
    };
    let Ok(formats) = (unsafe { data.EnumFormatEtc(DATADIR_GET.0 as u32) }) else {
        return mimes;
    };
    loop {
        let mut batch = [FORMATETC::default(); ENUMERATED];
        let mut fetched = 0u32;
        let result = unsafe { formats.Next(&mut batch, Some(&mut fetched)) };
        for format in &batch[..fetched as usize] {
            match format.cfFormat {
                text if text == CF_UNICODETEXT.0 => {
                    add(TEXT);
                    add("text/plain");
                }
                files if files == CF_HDROP.0 => add(URI_LIST),
                other => {
                    if let Some(name) = format_name(other).filter(|name| name.contains('/')) {
                        add(&name);
                    }
                }
            }
        }
        if result != S_OK || fetched == 0 {
            break;
        }
    }
    mimes
}

pub(crate) fn read(data: &IDataObject, mime: &str, limit: usize) -> Option<Vec<u8>> {
    if TEXT_TYPES.contains(&mime) {
        let wide = read_format(data, CF_UNICODETEXT.0, limit.saturating_mul(2))?;
        let units: Vec<u16> = wide
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .take_while(|unit| *unit != 0)
            .collect();
        return Some(String::from_utf16_lossy(&units).into_bytes());
    }
    let named = register(mime);
    if named != 0
        && let Some(bytes) = read_format(data, named, limit)
    {
        return Some(bytes);
    }
    if mime == URI_LIST {
        let dropped = read_format(data, CF_HDROP.0, limit)?;
        return Some(hdrop::decode(&dropped).join("\r\n").into_bytes());
    }
    None
}

fn read_format(data: &IDataObject, format: u16, limit: usize) -> Option<Vec<u8>> {
    let wanted = describe(format);
    if unsafe { data.QueryGetData(&wanted) } != S_OK {
        return None;
    }
    let mut medium = unsafe { data.GetData(&wanted) }
        .inspect_err(|error| tracing::debug!("drag data unavailable: {error}"))
        .ok()?;
    let bytes = (medium.tymed == TYMED_HGLOBAL.0 as u32)
        .then(|| copy_global(unsafe { medium.u.hGlobal }, limit))
        .flatten();
    unsafe { ReleaseStgMedium(&mut medium) };
    bytes
}

fn copy_global(handle: HGLOBAL, limit: usize) -> Option<Vec<u8>> {
    let size = unsafe { GlobalSize(handle) };
    if size > limit {
        tracing::debug!("drag data of {size} bytes exceeds {limit}");
        return None;
    }
    let address = unsafe { GlobalLock(handle) }.cast::<u8>();
    if address.is_null() {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(address, size) }.to_vec();
    unlock(handle);
    Some(bytes)
}

pub(crate) fn global_of(bytes: &[u8]) -> windows::core::Result<HGLOBAL> {
    let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)) }?;
    let address = unsafe { GlobalLock(handle) }.cast::<u8>();
    if address.is_null() {
        let error = windows::core::Error::from_thread();
        if let Err(freeing) = unsafe { GlobalFree(Some(handle)) } {
            tracing::debug!("failed to free drag memory: {freeing}");
        }
        return Err(error);
    }
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), address, bytes.len()) };
    unlock(handle);
    Ok(handle)
}

fn unlock(handle: HGLOBAL) {
    if let Err(error) = unsafe { GlobalUnlock(handle) }
        && error.code().is_err()
    {
        tracing::debug!("failed to unlock drag memory: {error}");
    }
}

fn format_name(format: u16) -> Option<String> {
    let mut name = [0u16; NAME_LENGTH];
    let length = unsafe { GetClipboardFormatNameW(u32::from(format), &mut name) };
    let length = usize::try_from(length).ok().filter(|length| *length > 0)?;
    Some(String::from_utf16_lossy(&name[..length]))
}
