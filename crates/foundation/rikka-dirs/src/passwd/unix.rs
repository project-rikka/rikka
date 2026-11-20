use std::ffi::CStr;
use std::path::PathBuf;

use super::Entry;

#[must_use]
pub fn current_uid() -> u32 {
    unsafe { libc::getuid() }
}

#[must_use]
pub fn current() -> Option<Entry> {
    lookup(current_uid())
}

const INITIAL_BUFFER_SIZE: usize = 1024;
const MAX_BUFFER_SIZE: usize = 64 * 1024;

pub fn lookup(uid: u32) -> Option<Entry> {
    let mut buffer_size = INITIAL_BUFFER_SIZE;
    loop {
        let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
        let mut buffer = vec![0 as libc::c_char; buffer_size];
        let mut found: *mut libc::passwd = std::ptr::null_mut();
        let code = unsafe {
            libc::getpwuid_r(
                uid,
                &mut entry,
                buffer.as_mut_ptr(),
                buffer.len(),
                &mut found,
            )
        };
        if code == libc::ERANGE && buffer_size < MAX_BUFFER_SIZE {
            buffer_size *= 2;
            continue;
        }
        if code != 0 || found.is_null() {
            return None;
        }
        let to_string = |field: *const libc::c_char| match field.is_null() {
            true => String::new(),
            false => unsafe { CStr::from_ptr(field) }
                .to_string_lossy()
                .into_owned(),
        };
        return Some(Entry {
            name: to_string(entry.pw_name),
            gecos: to_string(entry.pw_gecos),
            home: PathBuf::from(to_string(entry.pw_dir)),
        });
    }
}
