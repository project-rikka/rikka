use core::ffi::{CStr, c_char, c_void};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::compiler::Settings;
use crate::ffi::{
    E_NO_INTERFACE, E_NOT_FOUND, FileSystemVtbl, SlangResult, UUID_CASTABLE, UUID_FILE_SYSTEM,
    UUID_UNKNOWN, UnknownVtbl, Uuid, functions,
};

#[repr(C)]
pub(crate) struct Library {
    vtable: *const FileSystemVtbl,
    references: AtomicU32,
    settings: Arc<Settings>,
}

static VTABLE: FileSystemVtbl = FileSystemVtbl {
    unknown: UnknownVtbl {
        query_interface,
        add_ref,
        release,
    },
    cast_as,
    load_file,
};

impl Library {
    pub(crate) fn into_raw(settings: Arc<Settings>) -> *mut c_void {
        Box::into_raw(Box::new(Self {
            vtable: &VTABLE,
            references: AtomicU32::new(1),
            settings,
        }))
        .cast()
    }

    pub(crate) unsafe fn release(raw: *mut c_void) {
        unsafe { release(raw) };
    }

    fn find(&self, path: &str) -> Option<&str> {
        let mut parts: Vec<&str> = Vec::new();
        for part in path.split(['/', '\\']) {
            match part {
                "" | "." => {}
                ".." => {
                    parts.pop()?;
                }
                part => parts.push(part),
            }
        }
        self.settings
            .library
            .get(&parts.join("/"))
            .map(|text| &**text)
    }
}

fn implements(uuid: &Uuid) -> bool {
    [UUID_UNKNOWN, UUID_CASTABLE, UUID_FILE_SYSTEM].contains(uuid)
}

unsafe extern "C" fn query_interface(
    this: *mut c_void,
    uuid: *const Uuid,
    out: *mut *mut c_void,
) -> SlangResult {
    unsafe {
        if implements(&*uuid) {
            add_ref(this);
            *out = this;
            0
        } else {
            *out = core::ptr::null_mut();
            E_NO_INTERFACE
        }
    }
}

unsafe extern "C" fn add_ref(this: *mut c_void) -> u32 {
    let library = unsafe { &*this.cast::<Library>() };
    library.references.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "C" fn release(this: *mut c_void) -> u32 {
    let library = unsafe { &*this.cast::<Library>() };
    let remaining = library.references.fetch_sub(1, Ordering::AcqRel) - 1;
    if remaining == 0 {
        drop(unsafe { Box::from_raw(this.cast::<Library>()) });
    }
    remaining
}

unsafe extern "C" fn cast_as(this: *mut c_void, uuid: *const Uuid) -> *mut c_void {
    if unsafe { implements(&*uuid) } {
        this
    } else {
        core::ptr::null_mut()
    }
}

unsafe extern "C" fn load_file(
    this: *mut c_void,
    path: *const c_char,
    out: *mut *mut c_void,
) -> SlangResult {
    unsafe {
        *out = core::ptr::null_mut();
        let library = &*this.cast::<Library>();
        let Some(text) = CStr::from_ptr(path)
            .to_str()
            .ok()
            .and_then(|path| library.find(path))
            .filter(|text| !text.is_empty())
        else {
            return E_NOT_FOUND;
        };
        let blob = (functions().slang_createBlob)(text.as_ptr().cast(), text.len());
        if blob.is_null() {
            return E_NOT_FOUND;
        }
        *out = blob;
        0
    }
}
