use std::ffi::c_int;
use std::ptr::NonNull;

use opusic_sys as ffi;

use crate::encoder::{OPUS_SAMPLE_RATE, failure};
use crate::error::{Error, Result};

const LONGEST_FRAME: usize = OPUS_SAMPLE_RATE as usize * 120 / 1000;

pub struct OpusDecoder {
    raw: NonNull<ffi::OpusDecoder>,
    channels: usize,
}

unsafe impl Send for OpusDecoder {}

impl OpusDecoder {
    pub fn new(channels: u16) -> Result<OpusDecoder> {
        let channels = match channels {
            1 | 2 => usize::from(channels),
            other => {
                return Err(Error::Channels(other));
            }
        };
        let mut status: c_int = ffi::OPUS_OK;
        let raw = unsafe {
            ffi::opus_decoder_create(OPUS_SAMPLE_RATE as i32, channels as c_int, &mut status)
        };
        let raw = NonNull::new(raw).ok_or_else(|| failure(status))?;
        let decoder = OpusDecoder { raw, channels };
        if status != ffi::OPUS_OK {
            return Err(failure(status));
        }
        Ok(decoder)
    }

    #[must_use]
    pub fn channels(&self) -> usize {
        self.channels
    }

    #[must_use]
    pub fn samples(&self, packet: &[u8]) -> Option<usize> {
        if packet.is_empty() {
            return None;
        }
        let samples = unsafe {
            ffi::opus_packet_get_nb_samples(
                packet.as_ptr(),
                packet.len().min(i32::MAX as usize) as i32,
                OPUS_SAMPLE_RATE as i32,
            )
        };
        usize::try_from(samples).ok().filter(|samples| *samples > 0)
    }

    pub fn decode(&mut self, packet: &[u8], out: &mut Vec<f32>) -> Result<usize> {
        self.run(Some(packet), LONGEST_FRAME, Recovery::Plain, out)
    }

    pub fn recover(&mut self, next: &[u8], frames: usize, out: &mut Vec<f32>) -> Result<usize> {
        self.run(Some(next), frames.min(LONGEST_FRAME), Recovery::InBand, out)
    }

    pub fn conceal(&mut self, frames: usize, out: &mut Vec<f32>) -> Result<usize> {
        self.run(None, frames.min(LONGEST_FRAME), Recovery::Plain, out)
    }

    fn run(
        &mut self,
        packet: Option<&[u8]>,
        frames: usize,
        recovery: Recovery,
        out: &mut Vec<f32>,
    ) -> Result<usize> {
        let start = out.len();
        out.resize(start + frames * self.channels, 0.0);
        let (data, length) = match packet {
            Some(packet) => (packet.as_ptr(), packet.len().min(i32::MAX as usize) as i32),
            None => (std::ptr::null(), 0),
        };
        let decoded = unsafe {
            ffi::opus_decode_float(
                self.raw.as_ptr(),
                data,
                length,
                out[start..].as_mut_ptr(),
                frames as c_int,
                c_int::from(recovery == Recovery::InBand),
            )
        };
        if decoded < 0 {
            out.truncate(start);
            return Err(failure(decoded));
        }
        out.truncate(start + decoded as usize * self.channels);
        Ok(decoded as usize)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Recovery {
    Plain,
    InBand,
}

impl Drop for OpusDecoder {
    fn drop(&mut self) {
        unsafe { ffi::opus_decoder_destroy(self.raw.as_ptr()) }
    }
}

impl std::fmt::Debug for OpusDecoder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OpusDecoder")
            .field("channels", &self.channels)
            .finish_non_exhaustive()
    }
}
