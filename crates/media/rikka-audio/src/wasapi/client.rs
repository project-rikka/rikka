use std::os::windows::io::{AsHandle, AsRawHandle};

use rikka_worker::EventFd;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Media::Audio::{
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK,
    AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, IAudioClient, IAudioClient3, WAVEFORMATEX,
};
use windows::core::Interface;

use crate::error::Result;

use super::format::DeviceFormat;

const HNS_PER_SECOND: i64 = 10_000_000;
const PROCESS_LOOPBACK_BUFFER: i64 = HNS_PER_SECOND / 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    Render,
    Capture,
    Loopback,
    ProcessLoopback,
}

pub(super) struct Stream {
    pub(super) client: IAudioClient,
    pub(super) format: DeviceFormat,
    pub(super) buffer_frames: u32,
    pub(super) latency_frames: u32,
}

pub(super) fn open(
    mut activate: impl FnMut() -> windows::core::Result<IAudioClient>,
    format: DeviceFormat,
    mode: Mode,
    period: Option<u32>,
    event: &EventFd,
) -> Result<Stream> {
    let wave = format.wave();
    let pointer = (&raw const wave).cast::<WAVEFORMATEX>();
    let client = match low_latency(&mut activate, format, mode, period, pointer)? {
        Some(client) => client,
        None => {
            let client = activate()?;
            let buffer = match mode {
                Mode::ProcessLoopback => PROCESS_LOOPBACK_BUFFER,
                _ => {
                    let mut device_period = 0;
                    unsafe { client.GetDevicePeriod(Some(&mut device_period), None)? };
                    let wanted = period.map_or(0, |frames| hns(frames, format.rate));
                    2 * wanted.max(device_period)
                }
            };
            let loopback = match mode {
                Mode::Loopback | Mode::ProcessLoopback => AUDCLNT_STREAMFLAGS_LOOPBACK,
                Mode::Render | Mode::Capture => 0,
            };
            let flags = AUDCLNT_STREAMFLAGS_EVENTCALLBACK
                | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY
                | loopback;
            unsafe {
                client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, buffer, 0, pointer, None)?;
            }
            client
        }
    };
    let buffer_frames = unsafe { client.GetBufferSize()? };
    let latency_frames = match mode {
        Mode::ProcessLoopback => 0,
        _ => {
            let latency = unsafe { client.GetStreamLatency()? };
            frames(latency, format.rate)
        }
    };
    unsafe { client.SetEventHandle(HANDLE(event.as_handle().as_raw_handle()))? };
    Ok(Stream {
        client,
        format,
        buffer_frames,
        latency_frames,
    })
}

fn low_latency(
    activate: &mut impl FnMut() -> windows::core::Result<IAudioClient>,
    format: DeviceFormat,
    mode: Mode,
    period: Option<u32>,
    pointer: *const WAVEFORMATEX,
) -> Result<Option<IAudioClient>> {
    let (Mode::Render | Mode::Capture, true, Some(wanted)) = (mode, format.float, period) else {
        return Ok(None);
    };
    let client = activate()?;
    let Ok(engine) = client.cast::<IAudioClient3>() else {
        return Ok(None);
    };
    let (mut default, mut fundamental, mut least, mut most) = (0, 0, 0, 0);
    if let Err(error) = unsafe {
        engine.GetSharedModeEnginePeriod(
            pointer,
            &mut default,
            &mut fundamental,
            &mut least,
            &mut most,
        )
    } {
        tracing::debug!(%error, "audio engine offers no low-latency periods");
        return Ok(None);
    }
    if wanted >= default || fundamental == 0 {
        return Ok(None);
    }
    let chosen = wanted.div_ceil(fundamental) * fundamental;
    let chosen = chosen.clamp(least, most);
    match unsafe {
        engine.InitializeSharedAudioStream(AUDCLNT_STREAMFLAGS_EVENTCALLBACK, chosen, pointer, None)
    } {
        Ok(()) => Ok(Some(client)),
        Err(error) => {
            tracing::debug!(%error, chosen, "low-latency audio stream refused");
            Ok(None)
        }
    }
}

fn hns(frames: u32, rate: u32) -> i64 {
    i64::from(frames) * HNS_PER_SECOND / i64::from(rate.max(1))
}

fn frames(hns: i64, rate: u32) -> u32 {
    u32::try_from(hns.max(0) * i64::from(rate) / HNS_PER_SECOND).unwrap_or(u32::MAX)
}
