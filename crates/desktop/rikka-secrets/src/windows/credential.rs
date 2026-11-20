use std::io;

use windows::Win32::Foundation::{ERROR_NOT_FOUND, FILETIME};
use windows::Win32::Security::Credentials::{
    CRED_FLAGS, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIAL_ATTRIBUTEW, CREDENTIALW,
    CredDeleteW, CredEnumerateW, CredFree, CredReadW, CredWriteW,
};
use windows::core::{HSTRING, PWSTR};
use zeroize::Zeroizing;

pub(super) const LARGEST_BLOB: usize = 2560;

const LONGEST_NAME: usize = 512;

const PARTS: &str = "rikka.parts";
const CREATED: &str = "rikka.created";

const WINDOWS_TO_UNIX_SECONDS: u64 = 11_644_473_600;
const TICKS_PER_SECOND: u64 = 10_000_000;

pub(super) struct Stored {
    pub(super) target: String,
    pub(super) label: String,
    pub(super) blob: Zeroizing<Vec<u8>>,
    pub(super) parts: usize,
    pub(super) created: Option<u64>,
    pub(super) written: u64,
}

pub(super) struct Written<'a> {
    pub(super) target: &'a str,
    pub(super) label: &'a str,
    pub(super) blob: &'a [u8],
    pub(super) parts: Option<usize>,
    pub(super) created: Option<u64>,
}

struct Read(*mut CREDENTIALW);

impl Drop for Read {
    fn drop(&mut self) {
        unsafe {
            erase(&*self.0);
            CredFree(self.0.cast_const().cast());
        }
    }
}

struct Listed {
    items: *mut *mut CREDENTIALW,
    count: usize,
}

impl Listed {
    fn credentials(&self) -> impl Iterator<Item = &CREDENTIALW> {
        (0..self.count).map(|index| unsafe { &**self.items.add(index) })
    }
}

impl Drop for Listed {
    fn drop(&mut self) {
        unsafe {
            for credential in self.credentials() {
                erase(credential);
            }
            CredFree(self.items.cast_const().cast());
        }
    }
}

pub(super) fn read(target: &str) -> io::Result<Option<Stored>> {
    let mut credential = std::ptr::null_mut();
    match unsafe {
        CredReadW(
            &HSTRING::from(target),
            CRED_TYPE_GENERIC,
            None,
            &mut credential,
        )
    } {
        Ok(()) => {
            let read = Read(credential);
            Ok(Some(unsafe { stored(&*read.0) }))
        }
        Err(error) if error.code() == ERROR_NOT_FOUND.to_hresult() => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn enumerate(filter: &str) -> io::Result<Vec<Stored>> {
    let mut count = 0u32;
    let mut items = std::ptr::null_mut();
    match unsafe { CredEnumerateW(&HSTRING::from(filter), None, &mut count, &mut items) } {
        Ok(()) => {
            let listed = Listed {
                items,
                count: count as usize,
            };
            Ok(listed
                .credentials()
                .filter(|credential| credential.Type == CRED_TYPE_GENERIC)
                .map(|credential| unsafe { stored(credential) })
                .collect())
        }
        Err(error) if error.code() == ERROR_NOT_FOUND.to_hresult() => Ok(Vec::new()),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn write(written: &Written<'_>) -> io::Result<()> {
    let mut target = wide(written.target);
    let mut label = wide(&written.label.chars().take(LONGEST_NAME).collect::<String>());
    let mut values: Vec<(Vec<u16>, Vec<u8>)> = Vec::new();
    if let Some(parts) = written.parts {
        values.push((wide(PARTS), parts.to_string().into_bytes()));
    }
    if let Some(created) = written.created {
        values.push((wide(CREATED), created.to_string().into_bytes()));
    }
    let mut attributes: Vec<CREDENTIAL_ATTRIBUTEW> = values
        .iter_mut()
        .map(|(keyword, value)| CREDENTIAL_ATTRIBUTEW {
            Keyword: PWSTR(keyword.as_mut_ptr()),
            Flags: 0,
            ValueSize: value.len() as u32,
            Value: value.as_mut_ptr(),
        })
        .collect();
    let credential = CREDENTIALW {
        Flags: CRED_FLAGS(0),
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(target.as_mut_ptr()),
        CredentialBlobSize: written.blob.len() as u32,
        CredentialBlob: written.blob.as_ptr().cast_mut(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        AttributeCount: attributes.len() as u32,
        Attributes: match attributes.is_empty() {
            true => std::ptr::null_mut(),
            false => attributes.as_mut_ptr(),
        },
        UserName: PWSTR(label.as_mut_ptr()),
        ..CREDENTIALW::default()
    };
    unsafe { CredWriteW(&credential, 0) }.map_err(io::Error::from)
}

pub(super) fn delete(target: &str) -> io::Result<bool> {
    match unsafe { CredDeleteW(&HSTRING::from(target), CRED_TYPE_GENERIC, None) } {
        Ok(()) => Ok(true),
        Err(error) if error.code() == ERROR_NOT_FOUND.to_hresult() => Ok(false),
        Err(error) => Err(error.into()),
    }
}

unsafe fn stored(credential: &CREDENTIALW) -> Stored {
    let blob = match credential.CredentialBlob.is_null() {
        true => Vec::new(),
        false => unsafe {
            std::slice::from_raw_parts(
                credential.CredentialBlob,
                credential.CredentialBlobSize as usize,
            )
        }
        .to_vec(),
    };
    let attributes = match credential.Attributes.is_null() {
        true => &[][..],
        false => unsafe {
            std::slice::from_raw_parts(credential.Attributes, credential.AttributeCount as usize)
        },
    };
    let number = |keyword: &str| {
        attributes
            .iter()
            .find(|attribute| unsafe { text(attribute.Keyword) } == keyword)
            .filter(|attribute| !attribute.Value.is_null())
            .and_then(|attribute| {
                let value = unsafe {
                    std::slice::from_raw_parts(attribute.Value, attribute.ValueSize as usize)
                };
                std::str::from_utf8(value).ok()?.parse::<u64>().ok()
            })
    };
    Stored {
        target: unsafe { text(credential.TargetName) },
        label: unsafe { text(credential.UserName) },
        blob: Zeroizing::new(blob),
        parts: number(PARTS).map_or(1, |parts| parts.max(1) as usize),
        created: number(CREATED),
        written: unix_seconds(credential.LastWritten),
    }
}

unsafe fn text(text: PWSTR) -> String {
    match text.is_null() {
        true => String::new(),
        false => String::from_utf16_lossy(unsafe { text.as_wide() }),
    }
}

unsafe fn erase(credential: &CREDENTIALW) {
    if !credential.CredentialBlob.is_null() {
        unsafe {
            std::ptr::write_bytes(
                credential.CredentialBlob,
                0,
                credential.CredentialBlobSize as usize,
            );
        }
    }
}

fn unix_seconds(time: FILETIME) -> u64 {
    let ticks = (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
    (ticks / TICKS_PER_SECOND).saturating_sub(WINDOWS_TO_UNIX_SECONDS)
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
