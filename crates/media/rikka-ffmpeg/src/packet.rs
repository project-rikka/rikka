use crate::avcodec::{self, AVPacket};
use crate::avutil::{self, AVRational};
use crate::error::{Error, Result};

const SKIP_END: std::ops::Range<usize> = 4..8;

#[derive(Debug)]
pub struct Packet {
    ptr: *mut AVPacket,
}

unsafe impl Send for Packet {}

impl Packet {
    pub fn new() -> Result<Self> {
        let ptr = unsafe { avcodec::av_packet_alloc() };
        if ptr.is_null() {
            return Err(Error::Unsupported("av_packet_alloc failed".into()));
        }
        Ok(Self { ptr })
    }

    pub fn as_mut_ptr(&mut self) -> *mut AVPacket {
        self.ptr
    }

    pub fn stream_index(&self) -> i32 {
        unsafe { (*self.ptr).stream_index }
    }

    pub fn set_stream_index(&mut self, index: i32) {
        unsafe { (*self.ptr).stream_index = index };
    }

    pub fn is_key(&self) -> bool {
        unsafe { (*self.ptr).flags & avcodec::AV_PKT_FLAG_KEY != 0 }
    }

    pub fn stamp(&self) -> Option<i64> {
        let (pts, dts) = unsafe { ((*self.ptr).pts, (*self.ptr).dts) };
        [pts, dts]
            .into_iter()
            .find(|stamp| *stamp != avutil::AV_NOPTS_VALUE)
    }

    pub fn pts(&self) -> Option<i64> {
        let pts = unsafe { (*self.ptr).pts };
        (pts != avutil::AV_NOPTS_VALUE).then_some(pts)
    }

    pub fn dts(&self) -> Option<i64> {
        let dts = unsafe { (*self.ptr).dts };
        (dts != avutil::AV_NOPTS_VALUE).then_some(dts)
    }

    pub fn place(&mut self, pts: i64, dts: i64, duration: i64, keyframe: bool) {
        unsafe {
            (*self.ptr).pts = pts;
            (*self.ptr).dts = dts;
            (*self.ptr).duration = duration;
            (*self.ptr).flags = match keyframe {
                true => avcodec::AV_PKT_FLAG_KEY,
                false => 0,
            };
        }
    }

    pub fn keep_trailing_samples(&mut self) {
        let (entries, count) = unsafe { ((*self.ptr).side_data, (*self.ptr).side_data_elems) };
        if entries.is_null() {
            return;
        }
        let entries = unsafe { std::slice::from_raw_parts(entries, count.max(0) as usize) };
        for entry in entries {
            let skips = entry.r#type == avcodec::AV_PKT_DATA_SKIP_SAMPLES;
            if skips && entry.size >= SKIP_END.end && !entry.data.is_null() {
                unsafe {
                    entry
                        .data
                        .add(SKIP_END.start)
                        .write_bytes(0, SKIP_END.len())
                };
            }
        }
    }

    pub fn span(&self) -> i64 {
        unsafe { (*self.ptr).duration.max(0) }
    }

    pub fn set_span(&mut self, span: i64) {
        unsafe { (*self.ptr).duration = span.max(0) };
    }

    pub fn shift(&mut self, by: i64) {
        unsafe {
            for stamp in [&raw mut (*self.ptr).pts, &raw mut (*self.ptr).dts] {
                if *stamp != avutil::AV_NOPTS_VALUE {
                    *stamp -= by;
                }
            }
        }
    }

    pub fn rescale(&mut self, from: AVRational, to: AVRational) {
        unsafe { avcodec::av_packet_rescale_ts(self.ptr, from, to) };
    }

    pub fn forget_place(&mut self) {
        unsafe { (*self.ptr).pos = -1 };
    }

    pub fn bytes(&self) -> &[u8] {
        unsafe {
            let (data, size) = ((*self.ptr).data, (*self.ptr).size);
            if data.is_null() || size <= 0 {
                return &[];
            }
            std::slice::from_raw_parts(data, size as usize)
        }
    }

    pub fn with_borrowed_data<R>(&mut self, data: &[u8], body: impl FnOnce(&mut Packet) -> R) -> R {
        unsafe {
            (*self.ptr).data = data.as_ptr().cast_mut();
            (*self.ptr).size = data.len().min(i32::MAX as usize) as i32;
        }
        let result = body(self);
        unsafe {
            (*self.ptr).data = std::ptr::null_mut();
            (*self.ptr).size = 0;
        }
        result
    }

    pub fn unref(&mut self) {
        unsafe { avcodec::av_packet_unref(self.ptr) };
    }
}

impl Drop for Packet {
    fn drop(&mut self) {
        unsafe { avcodec::av_packet_free(&mut self.ptr) };
    }
}
