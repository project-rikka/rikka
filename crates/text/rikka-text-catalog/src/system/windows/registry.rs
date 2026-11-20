use std::ffi::OsString;
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_NO_MORE_ITEMS};
use windows::Win32::System::Environment::ExpandEnvironmentStringsW;
use windows::Win32::System::Registry::{
    HKEY, KEY_QUERY_VALUE, REG_EXPAND_SZ, REG_SZ, RegCloseKey, RegEnumValueW, RegOpenKeyExW,
    RegQueryInfoKeyW,
};
use windows::core::{PCWSTR, PWSTR, w};

const FONTS_KEY: PCWSTR = w!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts");

struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

pub(crate) fn font_files(root: HKEY, fonts: &Path) -> io::Result<Vec<PathBuf>> {
    let mut opened = HKEY::default();
    let status = unsafe { RegOpenKeyExW(root, FONTS_KEY, None, KEY_QUERY_VALUE, &raw mut opened) };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(Vec::new());
    }
    status.ok()?;
    let key = Key(opened);
    let (mut values, mut longest_name, mut longest_data) = (0u32, 0u32, 0u32);
    unsafe {
        RegQueryInfoKeyW(
            key.0,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(&raw mut values),
            Some(&raw mut longest_name),
            Some(&raw mut longest_data),
            None,
            None,
        )
    }
    .ok()?;
    let mut name = vec![0u16; longest_name as usize + 1];
    let mut data = vec![0u8; longest_data as usize + 2];
    let mut files = Vec::with_capacity(values as usize);
    for index in 0..values {
        let mut name_length = name.len() as u32;
        let mut data_length = data.len() as u32;
        let mut kind = 0u32;
        let status = unsafe {
            RegEnumValueW(
                key.0,
                index,
                Some(PWSTR(name.as_mut_ptr())),
                &raw mut name_length,
                None,
                Some(&raw mut kind),
                Some(data.as_mut_ptr()),
                Some(&raw mut data_length),
            )
        };
        if status == ERROR_NO_MORE_ITEMS {
            break;
        }
        status.ok()?;
        let text = wide(&data[..data_length as usize]);
        let value = match kind {
            kind if kind == REG_SZ.0 => text,
            kind if kind == REG_EXPAND_SZ.0 => expanded(&text)?,
            _ => continue,
        };
        if value.is_empty() {
            continue;
        }
        let path = PathBuf::from(OsString::from_wide(&value));
        files.push(match path.is_absolute() {
            true => path,
            false => fonts.join(path),
        });
    }
    Ok(files)
}

fn wide(bytes: &[u8]) -> Vec<u16> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| u16::from_le_bytes(pair))
        .take_while(|&unit| unit != 0)
        .collect()
}

fn expanded(text: &[u16]) -> io::Result<Vec<u16>> {
    let terminated: Vec<u16> = text.iter().copied().chain([0]).collect();
    let source = PCWSTR(terminated.as_ptr());
    let needed = unsafe { ExpandEnvironmentStringsW(source, None) };
    if needed == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut expanded = vec![0u16; needed as usize];
    let written = unsafe { ExpandEnvironmentStringsW(source, Some(&mut expanded)) };
    if written == 0 || written > needed {
        return Err(io::Error::last_os_error());
    }
    expanded.truncate(written as usize - 1);
    Ok(expanded)
}
