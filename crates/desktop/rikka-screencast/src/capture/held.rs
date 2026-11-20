use std::collections::HashMap;
use std::os::fd::{BorrowedFd, OwnedFd};
use std::sync::Arc;

use libspa::sys;
use pipewire::channel::Sender;
use pipewire::sys as pw_sys;
use rikka_gpu::DrmFormat;

use crate::frames::{BufferKey, Plane};

pub(crate) type Fds = Arc<[OwnedFd]>;

pub(crate) enum Command {
    Return(BufferKey),
    Offer(Vec<DrmFormat>),
}

pub(crate) struct Held {
    key: BufferKey,
    commands: Sender<Command>,
}

impl Held {
    pub(crate) fn new(key: BufferKey, commands: Sender<Command>) -> Held {
        Held { key, commands }
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        if self.commands.send(Command::Return(self.key)).is_err() {
            tracing::debug!("the screen capture ended before its buffer came back");
        }
    }
}

#[derive(Default)]
pub(crate) struct Buffers {
    added: u64,
    shared: HashMap<usize, (u64, Option<Fds>)>,
}

impl Buffers {
    pub(crate) fn add(&mut self, buffer: *mut pw_sys::pw_buffer) {
        self.added += 1;
        self.shared.insert(buffer as usize, (self.added, None));
    }

    pub(crate) fn remove(&mut self, buffer: *mut pw_sys::pw_buffer) {
        self.shared.remove(&(buffer as usize));
    }

    pub(crate) fn shared(&mut self, buffer: *mut pw_sys::pw_buffer) -> Option<(BufferKey, Fds)> {
        let (generation, kept) = self.shared.get_mut(&(buffer as usize))?;
        if kept.is_none() {
            *kept = unsafe { fds(buffer) };
        }
        Some((
            BufferKey {
                generation: *generation,
                buffer: buffer as usize,
            },
            Arc::clone(kept.as_ref()?),
        ))
    }

    pub(crate) fn returned(&self, key: BufferKey) -> Option<*mut pw_sys::pw_buffer> {
        let (generation, _) = self.shared.get(&key.buffer)?;
        (*generation == key.generation).then_some(key.buffer as *mut pw_sys::pw_buffer)
    }
}

pub(crate) unsafe fn corrupted(buffer: *mut pw_sys::pw_buffer) -> bool {
    let spa = unsafe { (*buffer).buffer };
    if spa.is_null() {
        return true;
    }
    let (metas, count) = unsafe { ((*spa).metas, (*spa).n_metas as usize) };
    if metas.is_null() {
        return false;
    }
    unsafe { std::slice::from_raw_parts(metas, count) }
        .iter()
        .filter(|meta| {
            meta.type_ == sys::SPA_META_Header
                && meta.size as usize >= size_of::<sys::spa_meta_header>()
                && !meta.data.is_null()
        })
        .any(|meta| {
            let flags = unsafe { (*meta.data.cast::<sys::spa_meta_header>()).flags };
            flags & sys::SPA_META_HEADER_FLAG_CORRUPTED != 0
        })
}

unsafe fn datas<'a>(buffer: *mut pw_sys::pw_buffer) -> Option<&'a [sys::spa_data]> {
    let spa = unsafe { (*buffer).buffer };
    if spa.is_null() || unsafe { (*spa).datas }.is_null() {
        return None;
    }
    let count = unsafe { (*spa).n_datas } as usize;
    let datas = unsafe { std::slice::from_raw_parts((*spa).datas, count) };
    let shared = !datas.is_empty() && datas.iter().all(|data| data.type_ == sys::SPA_DATA_DmaBuf);
    shared.then_some(datas)
}

unsafe fn fds(buffer: *mut pw_sys::pw_buffer) -> Option<Fds> {
    let datas = unsafe { datas(buffer) }?;
    datas
        .iter()
        .map(|data| {
            let raw = i32::try_from(data.fd).ok().filter(|fd| *fd >= 0)?;
            unsafe { BorrowedFd::borrow_raw(raw) }
                .try_clone_to_owned()
                .inspect_err(|error| tracing::warn!("cannot keep a shared screen buffer: {error}"))
                .ok()
        })
        .collect()
}

pub(crate) unsafe fn planes(buffer: *mut pw_sys::pw_buffer) -> Option<Vec<Plane>> {
    let datas = unsafe { datas(buffer) }?;
    Some(
        datas
            .iter()
            .map(|data| match data.chunk.is_null() {
                true => Plane {
                    offset: data.mapoffset,
                    stride: 0,
                },
                false => unsafe {
                    Plane {
                        offset: (*data.chunk).offset,
                        stride: (*data.chunk).stride.max(0) as u32,
                    }
                },
            })
            .collect(),
    )
}
