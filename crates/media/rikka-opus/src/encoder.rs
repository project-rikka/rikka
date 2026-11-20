use std::ffi::{CStr, c_int};
use std::ptr::NonNull;

use opusic_sys as ffi;

use crate::error::{Error, Result};

pub const OPUS_SAMPLE_RATE: u32 = 48_000;
pub const OPUS_MAX_PACKET: usize = 4000;

const MIN_BITRATE: u32 = 6_000;
const MAX_BITRATE: u32 = 510_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpusApplication {
    Voip,
    Audio,
    LowDelay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpusSignal {
    Voice,
    Music,
}

pub struct OpusEncoder {
    raw: NonNull<ffi::OpusEncoder>,
    channels: usize,
}

unsafe impl Send for OpusEncoder {}

impl OpusEncoder {
    pub fn new(application: OpusApplication, channels: u16) -> Result<OpusEncoder> {
        let channels = match channels {
            1 | 2 => usize::from(channels),
            other => {
                return Err(Error::Channels(other));
            }
        };
        let application = match application {
            OpusApplication::Voip => ffi::OPUS_APPLICATION_VOIP,
            OpusApplication::Audio => ffi::OPUS_APPLICATION_AUDIO,
            OpusApplication::LowDelay => ffi::OPUS_APPLICATION_RESTRICTED_LOWDELAY,
        };
        let mut status: c_int = ffi::OPUS_OK;
        let raw = unsafe {
            ffi::opus_encoder_create(
                OPUS_SAMPLE_RATE as i32,
                channels as c_int,
                application,
                &mut status,
            )
        };
        let raw = NonNull::new(raw).ok_or_else(|| failure(status))?;
        let encoder = OpusEncoder { raw, channels };
        if status != ffi::OPUS_OK {
            return Err(failure(status));
        }
        Ok(encoder)
    }

    #[must_use]
    pub fn channels(&self) -> usize {
        self.channels
    }

    pub fn set_signal(&mut self, signal: OpusSignal) -> Result<()> {
        let signal = match signal {
            OpusSignal::Voice => ffi::OPUS_SIGNAL_VOICE,
            OpusSignal::Music => ffi::OPUS_SIGNAL_MUSIC,
        };
        self.control(ffi::OPUS_SET_SIGNAL_REQUEST, signal)
    }

    pub fn set_bitrate(&mut self, bits_per_second: u32) -> Result<()> {
        let bitrate = bits_per_second.clamp(MIN_BITRATE, MAX_BITRATE) as c_int;
        self.control(ffi::OPUS_SET_BITRATE_REQUEST, bitrate)
    }

    pub fn set_in_band_fec(&mut self, on: bool) -> Result<()> {
        self.control(ffi::OPUS_SET_INBAND_FEC_REQUEST, c_int::from(on))
    }

    pub fn set_expected_loss(&mut self, percent: u8) -> Result<()> {
        self.control(
            ffi::OPUS_SET_PACKET_LOSS_PERC_REQUEST,
            c_int::from(percent.min(100)),
        )
    }

    pub fn encode(&mut self, pcm: &[f32], packet: &mut [u8]) -> Result<usize> {
        if pcm.is_empty() || !pcm.len().is_multiple_of(self.channels) {
            return Err(Error::PartialFrame {
                samples: pcm.len(),
                channels: self.channels,
            });
        }
        let frames = (pcm.len() / self.channels) as c_int;
        let result = unsafe {
            ffi::opus_encode_float(
                self.raw.as_ptr(),
                pcm.as_ptr(),
                frames,
                packet.as_mut_ptr(),
                packet.len().min(i32::MAX as usize) as i32,
            )
        };
        if result < 0 {
            return Err(failure(result));
        }
        Ok(result as usize)
    }

    fn control(&mut self, request: c_int, value: c_int) -> Result<()> {
        let status = unsafe { ffi::opus_encoder_ctl(self.raw.as_ptr(), request, value) };
        match status == ffi::OPUS_OK {
            true => Ok(()),
            false => Err(failure(status)),
        }
    }
}

impl Drop for OpusEncoder {
    fn drop(&mut self) {
        unsafe { ffi::opus_encoder_destroy(self.raw.as_ptr()) }
    }
}

impl std::fmt::Debug for OpusEncoder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OpusEncoder")
            .field("channels", &self.channels)
            .finish_non_exhaustive()
    }
}

pub(crate) fn failure(status: c_int) -> Error {
    let message = unsafe { ffi::opus_strerror(status) };
    if message.is_null() {
        return Error::Library(format!("error {status}"));
    }
    let message = unsafe { CStr::from_ptr(message) };
    Error::Library(message.to_string_lossy().into_owned())
}
