use std::time::{Duration, Instant};

use alsa::pcm::{Access, Frames, HwParams, PCM};
use alsa::{Direction, ValueOr};
use rikka_hifi_core::{DeviceId, Plan};

use crate::exclusive::formats;

const BUFFER_MICROS: u32 = 200_000;
const PERIOD_MICROS: u32 = 20_000;
const BUSY_TIMEOUT: Duration = Duration::from_secs(3);
const BUSY_RETRY_INTERVAL: Duration = Duration::from_millis(50);

pub(crate) struct PcmDevice {
    pub(crate) pcm: PCM,
    pub(crate) period: usize,
    pub(crate) period_time: Duration,
    pub(crate) buffer: Frames,
    pub(crate) can_pause: bool,
}

pub(crate) fn open(device: &DeviceId, plan: &Plan) -> Result<PcmDevice, alsa::Error> {
    let pcm = open_when_free(&device.pcm(), BUSY_TIMEOUT)?;
    {
        let hardware = HwParams::any(&pcm)?;
        hardware.set_access(Access::RWInterleaved)?;
        hardware.set_format(formats::alsa(plan.format))?;
        hardware.set_channels(u32::from(plan.channels))?;
        hardware.set_rate_resample(false)?;
        hardware.set_rate(plan.rate, ValueOr::Nearest)?;
        hardware.set_buffer_time_near(BUFFER_MICROS, ValueOr::Nearest)?;
        hardware.set_period_time_near(PERIOD_MICROS, ValueOr::Nearest)?;
        pcm.hw_params(&hardware)?;
    }
    let (period, buffer, can_pause) = {
        let current = pcm.hw_params_current()?;
        (
            current.get_period_size()?,
            current.get_buffer_size()?,
            current.can_pause(),
        )
    };
    {
        let software = pcm.sw_params_current()?;
        software.set_start_threshold((buffer - period).max(period))?;
        software.set_avail_min(period)?;
        pcm.sw_params(&software)?;
    }
    Ok(PcmDevice {
        pcm,
        period: usize::try_from(period).unwrap_or(1).max(1),
        period_time: Duration::from_secs_f64(period as f64 / f64::from(plan.rate.max(1))),
        buffer,
        can_pause,
    })
}

fn open_when_free(name: &str, timeout: Duration) -> Result<PCM, alsa::Error> {
    let until = Instant::now() + timeout;
    loop {
        match PCM::new(name, Direction::Playback, true) {
            Err(error) if error.errno() == libc::EBUSY && Instant::now() < until => {
                std::thread::sleep(BUSY_RETRY_INTERVAL);
            }
            result => return result,
        }
    }
}
