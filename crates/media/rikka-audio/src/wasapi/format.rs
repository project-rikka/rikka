use windows::Win32::Media::Audio::{WAVEFORMATEX, WAVEFORMATEXTENSIBLE, WAVEFORMATEXTENSIBLE_0};
use windows::Win32::Media::KernelStreaming::WAVE_FORMAT_EXTENSIBLE;
use windows::Win32::Media::Multimedia::{KSDATAFORMAT_SUBTYPE_IEEE_FLOAT, WAVE_FORMAT_IEEE_FLOAT};
use windows::Win32::System::Com::CoTaskMemFree;

use crate::error::{Result, Role};
use crate::frame::check_channels;

const FLOAT_BITS: u16 = 32;
const SAMPLE_BYTES: u16 = 4;
const FRONT_CENTER: u32 = 0x4;
const FRONT_PAIR: u32 = 0x3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DeviceFormat {
    pub(super) rate: u32,
    pub(super) channels: u16,
    pub(super) mask: u32,
    pub(super) float: bool,
}

impl DeviceFormat {
    pub(super) fn float32(rate: u32, channels: u16) -> DeviceFormat {
        DeviceFormat {
            rate,
            channels,
            mask: default_mask(channels),
            float: true,
        }
    }

    pub(super) fn check(&self, role: Role) -> Result<()> {
        check_channels(self.channels, role)
    }

    pub(super) fn wave(&self) -> WAVEFORMATEXTENSIBLE {
        let block = self.channels * SAMPLE_BYTES;
        WAVEFORMATEXTENSIBLE {
            Format: WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_EXTENSIBLE as u16,
                nChannels: self.channels,
                nSamplesPerSec: self.rate,
                nAvgBytesPerSec: self.rate * u32::from(block),
                nBlockAlign: block,
                wBitsPerSample: FLOAT_BITS,
                cbSize: (size_of::<WAVEFORMATEXTENSIBLE>() - size_of::<WAVEFORMATEX>()) as u16,
            },
            Samples: WAVEFORMATEXTENSIBLE_0 {
                wValidBitsPerSample: FLOAT_BITS,
            },
            dwChannelMask: self.mask,
            SubFormat: KSDATAFORMAT_SUBTYPE_IEEE_FLOAT,
        }
    }
}

pub(super) struct MixFormat(*mut WAVEFORMATEX);

impl MixFormat {
    pub(super) unsafe fn from_raw(format: *mut WAVEFORMATEX) -> MixFormat {
        MixFormat(format)
    }

    pub(super) fn read(&self) -> DeviceFormat {
        let header = unsafe { self.0.read_unaligned() };
        let tag = u32::from(header.wFormatTag);
        let extensible = tag == WAVE_FORMAT_EXTENSIBLE
            && usize::from(header.cbSize)
                >= size_of::<WAVEFORMATEXTENSIBLE>() - size_of::<WAVEFORMATEX>();
        let (mask, float) = match extensible {
            true => {
                let full = unsafe { self.0.cast::<WAVEFORMATEXTENSIBLE>().read_unaligned() };
                let subformat = full.SubFormat;
                (
                    full.dwChannelMask,
                    subformat == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT,
                )
            }
            false => (
                default_mask(header.nChannels),
                tag == WAVE_FORMAT_IEEE_FLOAT,
            ),
        };
        DeviceFormat {
            rate: header.nSamplesPerSec,
            channels: header.nChannels,
            mask: match mask {
                0 => default_mask(header.nChannels),
                mask => mask,
            },
            float: float && header.wBitsPerSample == FLOAT_BITS,
        }
    }
}

impl Drop for MixFormat {
    fn drop(&mut self) {
        unsafe { CoTaskMemFree(Some(self.0.cast_const().cast())) };
    }
}

fn default_mask(channels: u16) -> u32 {
    match channels {
        1 => FRONT_CENTER,
        2 => FRONT_PAIR,
        _ => 0,
    }
}
