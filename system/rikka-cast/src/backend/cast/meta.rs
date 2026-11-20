use std::os::fd::AsRawFd;
use std::time::Duration;

use libspa::sys;
use pipewire::sys as pw_sys;
use rikka_capture::{CaptureBuffer, PixelFormat, Region};

use crate::backend::cast::format::{self, CURSOR_EXTENT};

pub(crate) struct CursorPosition<'a> {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) hotspot_x: i32,
    pub(crate) hotspot_y: i32,
    pub(crate) bitmap: Option<CursorBitmap<'a>>,
}

pub(crate) struct CursorBitmap<'a> {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) format: PixelFormat,
    pub(crate) pixels: &'a [u8],
}

pub(crate) enum Cursor<'a> {
    NotTracked,
    Hidden,
    Visible(CursorPosition<'a>),
}

pub(crate) struct Frame<'a> {
    pub(crate) size: u32,
    pub(crate) stride: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) presented: Option<Duration>,
    pub(crate) sequence: u64,
    pub(crate) damage: &'a [Region],
    pub(crate) cursor: Cursor<'a>,
}

pub(crate) unsafe fn attach(buffer: *mut pw_sys::pw_buffer, capture: &CaptureBuffer) -> bool {
    let spa = unsafe { (*buffer).buffer };
    if spa.is_null() {
        return false;
    }
    let (datas, count) = unsafe { ((*spa).datas, (*spa).n_datas as usize) };
    if datas.is_null() || count == 0 {
        return false;
    }
    let planes = capture.planes();
    if planes.len() != count {
        return false;
    }
    let kind = match capture.is_dmabuf() {
        true => sys::SPA_DATA_DmaBuf,
        false => sys::SPA_DATA_MemFd,
    };

    for (index, plane) in planes.iter().enumerate() {
        let data = unsafe { &mut *datas.add(index) };
        data.type_ = kind;
        data.flags = sys::SPA_DATA_FLAG_READABLE;
        data.fd = i64::from(plane.fd.as_raw_fd());
        data.mapoffset = plane.offset;
        data.maxsize = plane.size as u32;
        data.data = std::ptr::null_mut();
        if !data.chunk.is_null() {
            let chunk = unsafe { &mut *data.chunk };
            chunk.offset = plane.offset;
            chunk.size = plane.size as u32;
            chunk.stride = plane.stride as i32;
            chunk.flags = 0;
        }
    }
    true
}

pub(crate) unsafe fn resolve_source(buffer: *mut pw_sys::pw_buffer, frame: &Frame<'_>) {
    let spa = unsafe { (*buffer).buffer };
    if spa.is_null() {
        return;
    }
    unsafe { chunk(spa, frame) };
    unsafe { header(spa, frame) };
    unsafe { damage(spa, frame) };
    match &frame.cursor {
        Cursor::NotTracked => {}
        Cursor::Hidden => unsafe { no_pointer(spa) },
        Cursor::Visible(position) => unsafe { pointer(spa, position) },
    }
}

pub(crate) unsafe fn empty(buffer: *mut pw_sys::pw_buffer) {
    let spa = unsafe { (*buffer).buffer };
    if spa.is_null() {
        return;
    }
    let data = unsafe { first(spa) };
    if let Some(data) = data
        && !data.chunk.is_null()
    {
        unsafe { (*data.chunk).size = 0 };
    }
}

unsafe fn first<'a>(spa: *mut sys::spa_buffer) -> Option<&'a mut sys::spa_data> {
    let (datas, count) = unsafe { ((*spa).datas, (*spa).n_datas) };
    if datas.is_null() || count == 0 {
        return None;
    }
    Some(unsafe { &mut *datas })
}

unsafe fn chunk(spa: *mut sys::spa_buffer, frame: &Frame<'_>) {
    let Some(data) = (unsafe { first(spa) }) else {
        return;
    };
    if data.chunk.is_null() {
        return;
    }
    let chunk = unsafe { &mut *data.chunk };
    chunk.size = frame.size;
    chunk.stride = frame.stride as i32;
    chunk.flags = 0;
}

unsafe fn header(spa: *mut sys::spa_buffer, frame: &Frame<'_>) {
    let header = unsafe {
        sys::spa_buffer_find_meta_data(spa, sys::SPA_META_Header, size_of::<sys::spa_meta_header>())
    }
    .cast::<sys::spa_meta_header>();
    if header.is_null() {
        return;
    }
    let header = unsafe { &mut *header };
    header.flags = 0;
    header.offset = 0;
    header.seq = frame.sequence;
    header.dts_offset = 0;
    header.pts = frame
        .presented
        .and_then(|at| i64::try_from(at.as_nanos()).ok())
        .unwrap_or(0);
}

unsafe fn damage(spa: *mut sys::spa_buffer, frame: &Frame<'_>) {
    let meta = unsafe { sys::spa_buffer_find_meta(spa, sys::SPA_META_VideoDamage) };
    if meta.is_null() {
        return;
    }
    let (at, size) = unsafe { ((*meta).data.cast::<sys::spa_meta_region>(), (*meta).size) };
    if at.is_null() {
        return;
    }
    let room = size as usize / size_of::<sys::spa_meta_region>();
    if room == 0 {
        return;
    }

    let whole = [Region::new(0, 0, frame.width, frame.height)];
    let regions = match frame.damage.is_empty() || frame.damage.len() > room - 1 {
        true => whole.as_slice(),
        false => frame.damage,
    };

    for (index, region) in regions.iter().enumerate() {
        unsafe {
            (*at.add(index)).region = sys::spa_region {
                position: sys::spa_point {
                    x: region.x,
                    y: region.y,
                },
                size: sys::spa_rectangle {
                    width: region.width,
                    height: region.height,
                },
            };
        }
    }
    for index in regions.len()..room {
        unsafe {
            (*at.add(index)).region = sys::spa_region {
                position: sys::spa_point { x: 0, y: 0 },
                size: sys::spa_rectangle {
                    width: 0,
                    height: 0,
                },
            };
        }
    }
}

unsafe fn pointer(spa: *mut sys::spa_buffer, spot: &CursorPosition<'_>) {
    let selection = format::cursor_meta_size();
    let cursor = unsafe { sys::spa_buffer_find_meta_data(spa, sys::SPA_META_Cursor, selection) }
        .cast::<sys::spa_meta_cursor>();
    if cursor.is_null() {
        return;
    }

    unsafe {
        (*cursor).id = 1;
        (*cursor).flags = 0;
        (*cursor).position = sys::spa_point {
            x: spot.x,
            y: spot.y,
        };
        (*cursor).hotspot = sys::spa_point {
            x: spot.hotspot_x,
            y: spot.hotspot_y,
        };
        (*cursor).bitmap_offset = 0;
    }

    let Some(bitmap) = &spot.bitmap else {
        return;
    };
    if bitmap.width == 0
        || bitmap.height == 0
        || bitmap.width > CURSOR_EXTENT
        || bitmap.height > CURSOR_EXTENT
    {
        return;
    }
    let stride = bitmap.width as usize * 4;
    let wanted_bytes = stride * bitmap.height as usize;
    if bitmap.pixels.len() < wanted_bytes {
        return;
    }

    let head = size_of::<sys::spa_meta_cursor>();
    unsafe {
        (*cursor).bitmap_offset = head as u32;
        let bits = cursor.cast::<u8>().add(head).cast::<sys::spa_meta_bitmap>();
        (*bits).format = format::video_format(bitmap.format).as_raw();
        (*bits).size = sys::spa_rectangle {
            width: bitmap.width,
            height: bitmap.height,
        };
        (*bits).stride = stride as i32;
        (*bits).offset = size_of::<sys::spa_meta_bitmap>() as u32;

        let pixels = bits.cast::<u8>().add(size_of::<sys::spa_meta_bitmap>());
        std::ptr::copy_nonoverlapping(bitmap.pixels.as_ptr(), pixels, wanted_bytes);
    }
}

unsafe fn no_pointer(spa: *mut sys::spa_buffer) {
    let cursor = unsafe {
        sys::spa_buffer_find_meta_data(spa, sys::SPA_META_Cursor, format::cursor_meta_size())
    }
    .cast::<sys::spa_meta_cursor>();
    if !cursor.is_null() {
        unsafe { (*cursor).id = 0 };
    }
}
