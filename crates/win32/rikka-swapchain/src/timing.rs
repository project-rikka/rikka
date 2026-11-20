use std::time::{Duration, Instant};

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DWM_TIMING_INFO, DwmGetCompositionTimingInfo};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Presented {
    pub at: Instant,
    pub refresh: Option<Duration>,
}

#[derive(Debug)]
pub(crate) struct Timing {
    frequency: i64,
    submitted: Option<i64>,
}

impl Timing {
    pub(crate) fn new() -> Self {
        let mut frequency = 0;
        if let Err(error) = unsafe { QueryPerformanceFrequency(&mut frequency) } {
            tracing::warn!("failed to read the performance counter frequency: {error}");
        }
        Self {
            frequency,
            submitted: None,
        }
    }

    pub(crate) fn submitted(&mut self) {
        self.submitted = counter();
    }

    pub(crate) fn take(&mut self) -> Option<Presented> {
        let submitted = self.submitted?;
        if self.frequency <= 0 {
            return None;
        }
        let composition = composition_timing()?;
        let shown = first_vblank_after(&composition, submitted)?;
        self.submitted = None;
        let refresh = (composition.qpcRefreshPeriod > 0)
            .then(|| self.duration(composition.qpcRefreshPeriod as i64));
        Some(Presented {
            at: self.instant(shown)?,
            refresh,
        })
    }

    fn duration(&self, ticks: i64) -> Duration {
        let nanos = i128::from(ticks.max(0)) * 1_000_000_000 / i128::from(self.frequency);
        Duration::from_nanos(u64::try_from(nanos).unwrap_or(u64::MAX))
    }

    fn instant(&self, at: i64) -> Option<Instant> {
        let now = counter()?;
        let reference = Instant::now();
        match now >= at {
            true => reference.checked_sub(self.duration(now - at)),
            false => reference.checked_add(self.duration(at - now)),
        }
    }
}

fn counter() -> Option<i64> {
    let mut now = 0;
    match unsafe { QueryPerformanceCounter(&mut now) } {
        Ok(()) => Some(now),
        Err(error) => {
            tracing::debug!("failed to read the performance counter: {error}");
            None
        }
    }
}

fn composition_timing() -> Option<DWM_TIMING_INFO> {
    let mut timing = DWM_TIMING_INFO {
        cbSize: size_of::<DWM_TIMING_INFO>() as u32,
        ..Default::default()
    };
    match unsafe { DwmGetCompositionTimingInfo(HWND::default(), &mut timing) } {
        Ok(()) => Some(timing),
        Err(error) => {
            tracing::debug!("failed to read the composition timing: {error}");
            None
        }
    }
}

fn first_vblank_after(timing: &DWM_TIMING_INFO, submitted: i64) -> Option<i64> {
    let vblank = timing.qpcVBlank as i64;
    let period = timing.qpcRefreshPeriod as i64;
    if period <= 0 || vblank < submitted {
        return None;
    }
    Some(vblank - (vblank - submitted) / period * period)
}
